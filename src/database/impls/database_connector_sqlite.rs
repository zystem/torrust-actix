use std::collections::BTreeMap;
use std::ops::Deref;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use async_std::task;
use futures_util::TryStreamExt;
use log::{error, info};
use sha1::{Digest, Sha1};
use sqlx::{ConnectOptions, Error, Sqlite, Pool, Row, Transaction};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use crate::config::structs::configuration::Configuration;
use crate::database::enums::database_drivers::DatabaseDrivers;
use crate::database::structs::database_connector::DatabaseConnector;
use crate::database::structs::database_connector_sqlite::DatabaseConnectorSQLite;
use crate::stats::enums::stats_event::StatsEvent;
use crate::tracker::enums::updates_action::UpdatesAction;
use crate::tracker::structs::info_hash::InfoHash;
use crate::tracker::structs::torrent_entry::TorrentEntry;
use crate::tracker::structs::torrent_tracker::TorrentTracker;
use crate::tracker::structs::user_entry_item::UserEntryItem;
use crate::tracker::structs::user_id::UserId;

impl DatabaseConnectorSQLite {
    #[tracing::instrument(level = "debug")]
    pub async fn create(dsl: &str) -> Result<Pool<Sqlite>, Error>
    {
        let options = SqliteConnectOptions::from_str(dsl)?
            .log_statements(log::LevelFilter::Debug)
            .log_slow_statements(log::LevelFilter::Debug, Duration::from_secs(1));
        SqlitePoolOptions::new().connect_with(options.create_if_missing(true)).await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn database_connector(config: Arc<Configuration>, create_database: bool) -> DatabaseConnector
    {
        let sqlite_connect = DatabaseConnectorSQLite::create(config.database.clone().path.as_str()).await;
        if sqlite_connect.is_err() {
            error!("[SQLite] Unable to connect to SQLite on DSL {}", config.database.clone().path);
            error!("[SQLite] Message: {:#?}", sqlite_connect.unwrap_err().into_database_error().unwrap().message());
            exit(1);
        }

        let mut structure = DatabaseConnector { mysql: None, sqlite: None, pgsql: None, engine: None };
        structure.sqlite = Some(DatabaseConnectorSQLite { pool: sqlite_connect.unwrap() });
        structure.engine = Some(DatabaseDrivers::sqlite3);

        if create_database {
            let pool = &structure.sqlite.clone().unwrap().pool;
            info!("[BOOT] Database creation triggered for SQLite.");

            info!("[BOOT SQLite] Setting the PRAGMA config...");
            let _ = sqlx::query("PRAGMA temp_store = memory;").execute(pool).await;
            let _ = sqlx::query("PRAGMA mmap_size = 30000000000;").execute(pool).await;
            let _ = sqlx::query("PRAGMA page_size = 32768;").execute(pool).await;
            let _ = sqlx::query("PRAGMA synchronous = full;").execute(pool).await;

            // Create Torrent DB
            info!("[BOOT SQLite] Creating table {}", config.database_structure.clone().torrents.table_name);
            match config.database_structure.clone().torrents.bin_type_infohash {
                true => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` BLOB PRIMARY KEY NOT NULL, `{}` INTEGER DEFAULT 0, `{}` INTEGER DEFAULT 0, `{}` INTEGER DEFAULT 0)",
                            config.database_structure.clone().torrents.table_name,
                            config.database_structure.clone().torrents.column_infohash,
                            config.database_structure.clone().torrents.column_seeds,
                            config.database_structure.clone().torrents.column_peers,
                            config.database_structure.clone().torrents.column_completed
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
                false => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL, `{}` INTEGER DEFAULT 0, `{}` INTEGER DEFAULT 0, `{}` INTEGER DEFAULT 0)",
                            config.database_structure.clone().torrents.table_name,
                            config.database_structure.clone().torrents.column_infohash,
                            config.database_structure.clone().torrents.column_seeds,
                            config.database_structure.clone().torrents.column_peers,
                            config.database_structure.clone().torrents.column_completed
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
            }

            // Create Whitelist DB
            info!("[BOOT SQLite] Creating table {}", config.database_structure.clone().whitelist.table_name);
            match config.database_structure.clone().whitelist.bin_type_infohash {
                true => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` BLOB PRIMARY KEY NOT NULL)",
                            config.database_structure.clone().whitelist.table_name,
                            config.database_structure.clone().whitelist.column_infohash
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
                false => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL)",
                            config.database_structure.clone().whitelist.table_name,
                            config.database_structure.clone().whitelist.column_infohash
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
            }

            // Create Blacklist DB
            info!("[BOOT SQLite] Creating table {}", config.database_structure.clone().blacklist.table_name);
            match config.database_structure.clone().blacklist.bin_type_infohash {
                true => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` BLOB PRIMARY KEY NOT NULL)",
                            config.database_structure.clone().blacklist.table_name,
                            config.database_structure.clone().blacklist.column_infohash
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
                false => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL)",
                            config.database_structure.clone().blacklist.table_name,
                            config.database_structure.clone().blacklist.column_infohash
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
            }

            // Create Keys DB
            info!("[BOOT SQLite] Creating table {}", config.database_structure.clone().keys.table_name);
            match config.database_structure.clone().keys.bin_type_hash {
                true => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` BLOB PRIMARY KEY NOT NULL, `{}` INTEGER DEFAULT 0)",
                            config.database_structure.clone().keys.table_name,
                            config.database_structure.clone().keys.column_hash,
                            config.database_structure.clone().keys.column_timeout
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
                false => {
                    match sqlx::query(
                        format!(
                            "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL, `{}` INTEGER DEFAULT 0)",
                            config.database_structure.clone().keys.table_name,
                            config.database_structure.clone().keys.column_hash,
                            config.database_structure.clone().keys.column_timeout
                        ).as_str()
                    ).execute(pool).await {
                        Ok(_) => {}
                        Err(error) => { panic!("[SQLite] Error: {}", error); }
                    }
                }
            }

            // Create Users DB
            info!("[BOOT SQLite] Creating table {}", config.database_structure.clone().users.table_name);
            match config.database_structure.clone().users.id_uuid {
                true => {
                    match config.database_structure.clone().users.bin_type_key {
                        true => {
                            match sqlx::query(
                                format!(
                                    "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL, `{}` BLOB NOT NULL, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0)",
                                    config.database_structure.clone().users.table_name,
                                    config.database_structure.clone().users.column_uuid,
                                    config.database_structure.clone().users.column_key,
                                    config.database_structure.clone().users.column_uploaded,
                                    config.database_structure.clone().users.column_downloaded,
                                    config.database_structure.clone().users.column_completed,
                                    config.database_structure.clone().users.column_active,
                                    config.database_structure.clone().users.column_updated
                                ).as_str()
                            ).execute(pool).await {
                                Ok(_) => {}
                                Err(error) => { panic!("[SQLite] Error: {}", error); }
                            }
                        }
                        false => {
                            match sqlx::query(
                                format!(
                                    "CREATE TABLE IF NOT EXISTS `{}` (`{}` TEXT PRIMARY KEY NOT NULL, `{}` TEXT NOT NULL, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0)",
                                    config.database_structure.clone().users.table_name,
                                    config.database_structure.clone().users.column_uuid,
                                    config.database_structure.clone().users.column_key,
                                    config.database_structure.clone().users.column_uploaded,
                                    config.database_structure.clone().users.column_downloaded,
                                    config.database_structure.clone().users.column_completed,
                                    config.database_structure.clone().users.column_active,
                                    config.database_structure.clone().users.column_updated
                                ).as_str()
                            ).execute(pool).await {
                                Ok(_) => {}
                                Err(error) => { panic!("[SQLite] Error: {}", error); }
                            }
                        }
                    }
                }
                false => {
                    match config.database_structure.clone().users.bin_type_key {
                        true => {
                            match sqlx::query(
                                format!(
                                    "CREATE TABLE IF NOT EXISTS `{}` (`{}` INTEGER PRIMARY KEY AUTOINCREMENT, `{}` BLOB NOT NULL, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0)",
                                    config.database_structure.clone().users.table_name,
                                    config.database_structure.clone().users.column_id,
                                    config.database_structure.clone().users.column_key,
                                    config.database_structure.clone().users.column_uploaded,
                                    config.database_structure.clone().users.column_downloaded,
                                    config.database_structure.clone().users.column_completed,
                                    config.database_structure.clone().users.column_active,
                                    config.database_structure.clone().users.column_updated
                                ).as_str()
                            ).execute(pool).await {
                                Ok(_) => {}
                                Err(error) => { panic!("[SQLite] Error: {}", error); }
                            }
                        }
                        false => {
                            match sqlx::query(
                                format!(
                                    "CREATE TABLE IF NOT EXISTS `{}` (`{}` INTEGER PRIMARY KEY AUTOINCREMENT, `{}` TEXT NOT NULL, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0, `{}` INTEGER NOT NULL DEFAULT 0)",
                                    config.database_structure.clone().users.table_name,
                                    config.database_structure.clone().users.column_id,
                                    config.database_structure.clone().users.column_key,
                                    config.database_structure.clone().users.column_uploaded,
                                    config.database_structure.clone().users.column_downloaded,
                                    config.database_structure.clone().users.column_completed,
                                    config.database_structure.clone().users.column_active,
                                    config.database_structure.clone().users.column_updated
                                ).as_str()
                            ).execute(pool).await {
                                Ok(_) => {}
                                Err(error) => { panic!("[SQLite] Error: {}", error); }
                            }
                        }
                    }
                }
            }
            info!("[BOOT] Created the database and tables, restart without the parameter to start the app.");
            task::sleep(Duration::from_secs(1)).await;
            exit(0);
        }

        structure
    }

    #[tracing::instrument(level = "debug")]
    pub async fn load_torrents(&self, tracker: Arc<TorrentTracker>) -> Result<(u64, u64), Error>
    {
        let mut start = 0u64;
        let length = 100000u64;
        let mut torrents = 0u64;
        let mut completed = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().torrents;
        loop {
            let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                true => {
                    format!(
                        "SELECT hex(`{}`) AS `{}`, `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.column_infohash,
                        structure.column_completed,
                        structure.table_name,
                        start,
                        length
                    )
                }
                false => {
                    format!(
                        "SELECT `{}`, `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.column_completed,
                        structure.table_name,
                        start,
                        length
                    )
                }
            };
            let mut rows = sqlx::query(string_format.as_str()).fetch(&self.pool);
            while let Some(result) = rows.try_next().await? {
                let info_hash_data: &[u8] = result.get(structure.column_infohash.as_str());
                let info_hash: [u8; 20] = <[u8; 20]>::try_from(hex::decode(info_hash_data).unwrap()[0..20].as_ref()).unwrap();
                let completed_count: u32 = result.get(structure.column_completed.as_str());
                tracker.add_torrent(
                    InfoHash(info_hash),
                    TorrentEntry {
                        seeds: BTreeMap::new(),
                        peers: BTreeMap::new(),
                        completed: completed_count as u64,
                        updated: std::time::Instant::now()
                    }
                );
                torrents += 1;
                completed += completed_count as u64;
            }
            start += length;
            if torrents < start {
                break;
            }
            info!("[SQLite] Handled {} torrents", torrents);
        }
        tracker.set_stats(StatsEvent::Completed, completed as i64);
        info!("[SQLite] Loaded {} torrents with {} completed", torrents, completed);
        Ok((torrents, completed))
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save_torrents(&self, tracker: Arc<TorrentTracker>, torrents: BTreeMap<InfoHash, (TorrentEntry, UpdatesAction)>) -> Result<(), Error>
    {
        let mut torrents_transaction = self.pool.begin().await?;
        let mut torrents_handled_entries = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().torrents;
        for (info_hash, (torrent_entry, updates_action)) in torrents.iter() {
            torrents_handled_entries += 1;
            match updates_action {
                UpdatesAction::Remove => {
                    if tracker.config.deref().clone().database.remove_action {
                        let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                            true => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`=X'{}'",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                            false => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                        };
                        match sqlx::query(string_format.as_str()).execute(&mut *torrents_transaction).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("[SQLite] Error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                UpdatesAction::Add | UpdatesAction::Update => {
                    match tracker.config.deref().clone().database.insert_vacant {
                        true => {
                            if tracker.config.deref().clone().database.update_peers {
                                let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                                    true => {
                                        format!(
                                            "INSERT INTO `{}` (`{}`, `{}`, `{}`) VALUES (X'{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                            structure.table_name,
                                            structure.column_infohash,
                                            structure.column_seeds,
                                            structure.column_peers,
                                            info_hash,
                                            torrent_entry.seeds.len(),
                                            torrent_entry.peers.len(),
                                            structure.column_infohash,
                                            structure.column_seeds,
                                            structure.column_seeds,
                                            structure.column_peers,
                                            structure.column_peers
                                        )
                                    }
                                    false => {
                                        format!(
                                            "INSERT INTO `{}` (`{}`, `{}`, `{}`) VALUES ('{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                            structure.table_name,
                                            structure.column_infohash,
                                            structure.column_seeds,
                                            structure.column_peers,
                                            info_hash,
                                            torrent_entry.seeds.len(),
                                            torrent_entry.peers.len(),
                                            structure.column_infohash,
                                            structure.column_seeds,
                                            structure.column_seeds,
                                            structure.column_peers,
                                            structure.column_peers
                                        )
                                    }
                                };
                                match sqlx::query(string_format.as_str()).execute(&mut *torrents_transaction).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("[SQLite] Error: {}", e);
                                        return Err(e);
                                    }
                                }
                            }
                            if tracker.config.deref().clone().database.update_completed {
                                let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                                    true => {
                                        format!(
                                            "INSERT INTO `{}` (`{}`, `{}`) VALUES (X'{}', {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`",
                                            structure.table_name,
                                            structure.column_infohash,
                                            structure.column_completed,
                                            info_hash,
                                            torrent_entry.completed,
                                            structure.column_infohash,
                                            structure.column_completed,
                                            structure.column_completed
                                        )
                                    }
                                    false => {
                                        format!(
                                            "INSERT INTO `{}` (`{}`, `{}`) VALUES ('{}', {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`",
                                            structure.table_name,
                                            structure.column_infohash,
                                            structure.column_completed,
                                            info_hash,
                                            torrent_entry.completed,
                                            structure.column_infohash,
                                            structure.column_completed,
                                            structure.column_completed
                                        )
                                    }
                                };
                                match sqlx::query(string_format.as_str()).execute(&mut *torrents_transaction).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("[SQLite] Error: {}", e);
                                        return Err(e);
                                    }
                                }
                            }
                        }
                        false => {
                            if tracker.config.deref().clone().database.update_peers {
                                let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                                    true => {
                                        format!(
                                            "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={} WHERE `{}`=X'{}'",
                                            structure.table_name,
                                            structure.column_seeds,
                                            torrent_entry.seeds.len(),
                                            structure.column_peers,
                                            torrent_entry.peers.len(),
                                            structure.column_infohash,
                                            info_hash
                                        )
                                    }
                                    false => {
                                        format!(
                                            "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={} WHERE `{}`='{}'",
                                            structure.table_name,
                                            structure.column_seeds,
                                            torrent_entry.seeds.len(),
                                            structure.column_peers,
                                            torrent_entry.peers.len(),
                                            structure.column_infohash,
                                            info_hash
                                        )
                                    }
                                };
                                match sqlx::query(string_format.as_str()).execute(&mut *torrents_transaction).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("[SQLite] Error: {}", e);
                                        return Err(e);
                                    }
                                }
                            }
                            if tracker.config.deref().clone().database.update_completed {
                                let string_format = match tracker.config.deref().clone().database_structure.torrents.bin_type_infohash {
                                    true => {
                                        format!(
                                            "UPDATE IGNORE `{}` SET `{}`={} WHERE `{}`=X'{}'",
                                            structure.table_name,
                                            structure.column_completed,
                                            torrent_entry.completed,
                                            structure.column_infohash,
                                            info_hash
                                        )
                                    }
                                    false => {
                                        format!(
                                            "UPDATE IGNORE `{}` SET `{}`={} WHERE `{}`='{}'",
                                            structure.table_name,
                                            structure.column_completed,
                                            torrent_entry.completed,
                                            structure.column_infohash,
                                            info_hash
                                        )
                                    }
                                };
                                match sqlx::query(string_format.as_str()).execute(&mut *torrents_transaction).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("[SQLite] Error: {}", e);
                                        return Err(e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if (torrents_handled_entries as f64 / 1000f64).fract() == 0.0 || torrents.len() as u64 == torrents_handled_entries {
                info!("[SQLite] Handled {} torrents", torrents_handled_entries);
            }
        }
        info!("[SQLite] Handled {} torrents", torrents_handled_entries);
        self.commit(torrents_transaction).await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn load_whitelist(&self, tracker: Arc<TorrentTracker>) -> Result<u64, Error>
    {
        let mut start = 0u64;
        let length = 100000u64;
        let mut hashes = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().whitelist;
        loop {
            let string_format = match tracker.config.deref().clone().database_structure.whitelist.bin_type_infohash {
                true => {
                    format!(
                        "SELECT HEX(`{}`) AS `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.column_infohash,
                        structure.table_name,
                        start,
                        length
                    )
                }
                false => {
                    format!(
                        "SELECT `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.table_name,
                        start,
                        length
                    )
                }
            };
            let mut rows = sqlx::query(string_format.as_str()).fetch(&self.pool);
            while let Some(result) = rows.try_next().await? {
                let info_hash_data: &[u8] = result.get(structure.column_infohash.as_str());
                let info_hash: [u8; 20] = <[u8; 20]>::try_from(hex::decode(info_hash_data).unwrap()[0..20].as_ref()).unwrap();
                tracker.add_whitelist(InfoHash(info_hash));
                hashes += 1;
            }
            start += length;
            if hashes < start {
                break;
            }
            info!("[SQLite] Handled {} whitelisted torrents", hashes);
        }
        info!("[SQLite] Handled {} whitelisted torrents", hashes);
        Ok(hashes)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save_whitelist(&self, tracker: Arc<TorrentTracker>, whitelists: Vec<(InfoHash, UpdatesAction)>) -> Result<u64, Error>
    {
        let mut whitelist_transaction = self.pool.begin().await?;
        let mut whitelist_handled_entries = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().whitelist;
        for (info_hash, updates_action) in whitelists.iter() {
            whitelist_handled_entries += 1;
            match updates_action {
                UpdatesAction::Remove => {
                    if tracker.config.deref().clone().database.remove_action {
                        let string_format = match tracker.config.deref().clone().database_structure.whitelist.bin_type_infohash {
                            true => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`=UNHEX('{}')",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                            false => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                        };
                        match sqlx::query(string_format.as_str()).execute(&mut *whitelist_transaction).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("[SQLite] Error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                UpdatesAction::Add | UpdatesAction::Update => {
                    let string_format = match tracker.config.deref().clone().database_structure.whitelist.bin_type_infohash {
                        true => {
                            format!(
                                "INSERT OR IGNORE INTO `{}` (`{}`) VALUES (X'{}')",
                                structure.table_name,
                                structure.column_infohash,
                                info_hash
                            )
                        }
                        false => {
                            format!(
                                "INSERT OR IGNORE INTO `{}` (`{}`) VALUES ('{}')",
                                structure.table_name,
                                structure.column_infohash,
                                info_hash
                            )
                        }
                    };
                    match sqlx::query(string_format.as_str()).execute(&mut *whitelist_transaction).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("[SQLite] Error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
            if (whitelist_handled_entries as f64 / 1000f64).fract() == 0.0 {
                info!("[SQLite] Handled {} whitelisted torrents", whitelist_handled_entries);
            }
        }
        info!("[SQLite] Handled {} whitelisted torrents", whitelist_handled_entries);
        let _ = self.commit(whitelist_transaction).await;
        Ok(whitelist_handled_entries)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn load_blacklist(&self, tracker: Arc<TorrentTracker>) -> Result<u64, Error>
    {
        let mut start = 0u64;
        let length = 100000u64;
        let mut hashes = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().blacklist;
        loop {
            let string_format = match tracker.config.deref().clone().database_structure.blacklist.bin_type_infohash {
                true => {
                    format!(
                        "SELECT HEX(`{}`) AS `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.column_infohash,
                        structure.table_name,
                        start,
                        length
                    )
                }
                false => {
                    format!(
                        "SELECT `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_infohash,
                        structure.table_name,
                        start,
                        length
                    )
                }
            };
            let mut rows = sqlx::query(string_format.as_str()).fetch(&self.pool);
            while let Some(result) = rows.try_next().await? {
                let info_hash_data: &[u8] = result.get(structure.column_infohash.as_str());
                let info_hash: [u8; 20] = <[u8; 20]>::try_from(hex::decode(info_hash_data).unwrap()[0..20].as_ref()).unwrap();
                tracker.add_blacklist(InfoHash(info_hash));
                hashes += 1;
            }
            start += length;
            if hashes < start {
                break;
            }
            info!("[SQLite] Handled {} blacklisted torrents", hashes);
        }
        info!("[SQLite] Handled {} blacklisted torrents", hashes);
        Ok(hashes)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save_blacklist(&self, tracker: Arc<TorrentTracker>, blacklists: Vec<(InfoHash, UpdatesAction)>) -> Result<u64, Error>
    {
        let mut blacklist_transaction = self.pool.begin().await?;
        let mut blacklist_handled_entries = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().blacklist;
        for (info_hash, updates_action) in blacklists.iter() {
            blacklist_handled_entries += 1;
            match updates_action {
                UpdatesAction::Remove => {
                    if tracker.config.deref().clone().database.remove_action {
                        let string_format = match tracker.config.deref().clone().database_structure.blacklist.bin_type_infohash {
                            true => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`=X'{}'",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                            false => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_infohash,
                                    info_hash
                                )
                            }
                        };
                        match sqlx::query(string_format.as_str()).execute(&mut *blacklist_transaction).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("[SQLite] Error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                UpdatesAction::Add | UpdatesAction::Update => {
                    let string_format = match tracker.config.deref().clone().database_structure.blacklist.bin_type_infohash {
                        true => {
                            format!(
                                "INSERT OR IGNORE INTO `{}` (`{}`) VALUES (X'{}')",
                                structure.table_name,
                                structure.column_infohash,
                                info_hash
                            )
                        }
                        false => {
                            format!(
                                "INSERT OR IGNORE INTO `{}` (`{}`) VALUES ('{}')",
                                structure.table_name,
                                structure.column_infohash,
                                info_hash
                            )
                        }
                    };
                    match sqlx::query(string_format.as_str()).execute(&mut *blacklist_transaction).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("[SQLite] Error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
            if (blacklist_handled_entries as f64 / 1000f64).fract() == 0.0 {
                info!("[SQLite] Handled {} blacklisted torrents", blacklist_handled_entries);
            }
        }
        info!("[SQLite] Handled {} blacklisted torrents", blacklist_handled_entries);
        let _ = self.commit(blacklist_transaction).await;
        Ok(blacklist_handled_entries)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn load_keys(&self, tracker: Arc<TorrentTracker>) -> Result<u64, Error>
    {
        let mut start = 0u64;
        let length = 100000u64;
        let mut hashes = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().keys;
        loop {
            let string_format = match tracker.config.deref().clone().database_structure.keys.bin_type_hash {
                true => {
                    format!(
                        "SELECT HEX(`{}`) AS `{}`, `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_hash,
                        structure.column_hash,
                        structure.column_timeout,
                        structure.table_name,
                        start,
                        length
                    )
                }
                false => {
                    format!(
                        "SELECT `{}`, `{}` FROM `{}` LIMIT {}, {}",
                        structure.column_hash,
                        structure.column_timeout,
                        structure.table_name,
                        start,
                        length
                    )
                }
            };
            let mut rows = sqlx::query(string_format.as_str()).fetch(&self.pool);
            while let Some(result) = rows.try_next().await? {
                let hash_data: &[u8] = result.get(structure.column_hash.as_str());
                let hash: [u8; 20] = <[u8; 20]>::try_from(hex::decode(hash_data).unwrap()[0..20].as_ref()).unwrap();
                let timeout: i64 = result.get(structure.column_timeout.as_str());
                tracker.add_key(InfoHash(hash), timeout);
                hashes += 1;
            }
            start += length;
            if hashes < start {
                break;
            }
            info!("[SQLite] Handled {} keys", hashes);
        }
        info!("[SQLite] Handled {} keys", hashes);
        Ok(hashes)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save_keys(&self, tracker: Arc<TorrentTracker>, keys: BTreeMap<InfoHash, (i64, UpdatesAction)>) -> Result<u64, Error>
    {
        let mut keys_transaction = self.pool.begin().await?;
        let mut keys_handled_entries = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().keys;
        for (hash, (timeout, update_action)) in keys.iter() {
            keys_handled_entries += 1;
            match update_action {
                UpdatesAction::Remove => {
                    if tracker.config.deref().clone().database.remove_action {
                        let string_format = match tracker.config.deref().clone().database_structure.keys.bin_type_hash {
                            true => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`=X'{}'",
                                    structure.table_name,
                                    structure.column_hash,
                                    hash
                                )
                            }
                            false => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_hash,
                                    hash
                                )
                            }
                        };
                        match sqlx::query(string_format.as_str()).execute(&mut *keys_transaction).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("[SQLite] Error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                UpdatesAction::Add | UpdatesAction::Update => {
                    let string_format = match tracker.config.deref().clone().database_structure.keys.bin_type_hash {
                        true => {
                            format!(
                                "INSERT INTO `{}` (`{}`, `{}`) VALUES (X'{}', {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`",
                                structure.table_name,
                                structure.column_hash,
                                structure.column_timeout,
                                hash,
                                timeout,
                                structure.column_hash,
                                structure.column_timeout,
                                structure.column_timeout
                            )
                        }
                        false => {
                            format!(
                                "INSERT INTO `{}` (`{}`, `{}`) VALUES ('{}', {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`",
                                structure.table_name,
                                structure.column_hash,
                                structure.column_timeout,
                                hash,
                                timeout,
                                structure.column_hash,
                                structure.column_timeout,
                                structure.column_timeout
                            )
                        }
                    };
                    match sqlx::query(string_format.as_str()).execute(&mut *keys_transaction).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("[SQLite] Error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
            if (keys_handled_entries as f64 / 1000f64).fract() == 0.0 {
                info!("[SQLite] Handled {} keys", keys_handled_entries);
            }
        }
        info!("[SQLite] Handled {} keys", keys_handled_entries);
        let _ = self.commit(keys_transaction).await;
        Ok(keys_handled_entries)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn load_users(&self, tracker: Arc<TorrentTracker>) -> Result<u64, Error>
    {
        let mut start = 0u64;
        let length = 100000u64;
        let mut hashes = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().users;
        loop {
            let string_format = match tracker.config.deref().clone().database_structure.users.id_uuid {
                true => {
                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                        true => {
                            format!(
                                "SELECT `{}`, HEX(`{}`) AS `{}`, `{}`, `{}`, `{}`, `{}`, `{}` FROM `{}` LIMIT {}, {}",
                                structure.column_uuid,
                                structure.column_key,
                                structure.column_key,
                                structure.column_uploaded,
                                structure.column_downloaded,
                                structure.column_completed,
                                structure.column_updated,
                                structure.column_active,
                                structure.table_name,
                                start,
                                length
                            )
                        }
                        false => {
                            format!(
                                "SELECT `{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}` FROM `{}` LIMIT {}, {}",
                                structure.column_uuid,
                                structure.column_key,
                                structure.column_uploaded,
                                structure.column_downloaded,
                                structure.column_completed,
                                structure.column_updated,
                                structure.column_active,
                                structure.table_name,
                                start,
                                length
                            )
                        }
                    }
                }
                false => {
                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                        true => {
                            format!(
                                "SELECT `{}`, HEX(`{}`) AS `{}`, `{}`, `{}`, `{}`, `{}`, `{}` FROM `{}` LIMIT {}, {}",
                                structure.column_id,
                                structure.column_key,
                                structure.column_key,
                                structure.column_uploaded,
                                structure.column_downloaded,
                                structure.column_completed,
                                structure.column_updated,
                                structure.column_active,
                                structure.table_name,
                                start,
                                length
                            )
                        }
                        false => {
                            format!(
                                "SELECT `{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}` FROM `{}` LIMIT {}, {}",
                                structure.column_id,
                                structure.column_key,
                                structure.column_uploaded,
                                structure.column_downloaded,
                                structure.column_completed,
                                structure.column_updated,
                                structure.column_active,
                                structure.table_name,
                                start,
                                length
                            )
                        }
                    }
                }
            };
            let mut rows = sqlx::query(string_format.as_str()).fetch(&self.pool);
            while let Some(result) = rows.try_next().await? {
                let hash = match tracker.config.deref().clone().database_structure.users.id_uuid {
                    true => {
                        let uuid_data: &[u8] = result.get(structure.column_uuid.as_str());
                        let mut hasher = Sha1::new();
                        hasher.update(uuid_data);
                        let hashed = <[u8; 20]>::try_from(hasher.finalize().as_slice()).unwrap();
                        hashed
                    }
                    false => {
                        let id_data: &[u8] = result.get(structure.column_id.as_str());
                        let mut hasher = Sha1::new();
                        hasher.update(id_data);
                        let hashed = <[u8; 20]>::try_from(hasher.finalize().as_slice()).unwrap();
                        hashed
                    }
                };
                tracker.add_user(UserId(hash), UserEntryItem {
                    key: UserId::from_str(result.get(structure.column_key.as_str())).unwrap(),
                    user_id: match tracker.config.deref().clone().database_structure.users.id_uuid {
                        true => { None }
                        false => { Some(result.get::<u32, &str>(structure.column_id.as_str()) as u64) }
                    },
                    user_uuid: match tracker.config.deref().clone().database_structure.users.id_uuid {
                        true => { Some(result.get(structure.column_uuid.as_str())) }
                        false => { None }
                    },
                    uploaded: result.get::<u32, &str>(structure.column_uploaded.as_str()) as u64,
                    downloaded: result.get::<u32, &str>(structure.column_downloaded.as_str()) as u64,
                    completed: result.get::<u32, &str>(structure.column_completed.as_str()) as u64,
                    updated: result.get::<u32, &str>(structure.column_updated.as_str()) as u64,
                    active: result.get::<i8, &str>(structure.column_active.as_str()) as u8,
                    torrents_active: Default::default(),
                });
                hashes += 1;
            }
            start += length;
            if hashes < start {
                break;
            }
            info!("[SQLite] Handled {} users", hashes);
        }
        info!("[SQLite] Handled {} users", hashes);
        Ok(hashes)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save_users(&self, tracker: Arc<TorrentTracker>, users: BTreeMap<UserId, (UserEntryItem, UpdatesAction)>) -> Result<(), Error>
    {
        let mut users_transaction = self.pool.begin().await?;
        let mut users_handled_entries = 0u64;
        let structure = tracker.config.deref().clone().database_structure.clone().users;
        for (_, (user_entry_item, updates_action)) in users.iter() {
            users_handled_entries += 1;
            match updates_action {
                UpdatesAction::Remove => {
                    if tracker.config.deref().clone().database.remove_action {
                        let string_format = match tracker.config.deref().clone().database_structure.users.id_uuid {
                            true => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_uuid,
                                    user_entry_item.user_uuid.clone().unwrap()
                                )
                            }
                            false => {
                                format!(
                                    "DELETE FROM `{}` WHERE `{}`='{}'",
                                    structure.table_name,
                                    structure.column_id,
                                    user_entry_item.user_id.unwrap()
                                )
                            }
                        };
                        match sqlx::query(string_format.as_str()).execute(&mut *users_transaction).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("[SQLite] Error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                UpdatesAction::Add | UpdatesAction::Update => {
                    let string_format = match  tracker.config.deref().clone().database.insert_vacant {
                        true => {
                            match tracker.config.deref().clone().database_structure.users.id_uuid {
                                true => {
                                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                                        true => {
                                            format!(
                                                "INSERT INTO `{}` (`{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}`) VALUES ('{}', {}, {}, {}, X'{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                                structure.table_name,
                                                structure.column_uuid,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                user_entry_item.user_uuid.clone().unwrap() ,
                                                user_entry_item.completed,
                                                user_entry_item.active,
                                                user_entry_item.downloaded,
                                                user_entry_item.key,
                                                user_entry_item.uploaded,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                structure.column_completed,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                structure.column_updated
                                            )
                                        }
                                        false => {
                                            format!(
                                                "INSERT INTO `{}` (`{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}`) VALUES ('{}', {}, {}, {}, '{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                                structure.table_name,
                                                structure.column_uuid,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                user_entry_item.user_uuid.clone().unwrap(),
                                                user_entry_item.completed,
                                                user_entry_item.active,
                                                user_entry_item.downloaded,
                                                user_entry_item.key,
                                                user_entry_item.uploaded,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                structure.column_completed,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                structure.column_updated
                                            )
                                        }
                                    }
                                }
                                false => {
                                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                                        true => {
                                            format!(
                                                "INSERT INTO `{}` (`{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}`) VALUES (X'{}', {}, {}, {}, X'{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                                structure.table_name,
                                                structure.column_id,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                user_entry_item.user_uuid.clone().unwrap(),
                                                user_entry_item.completed,
                                                user_entry_item.active,
                                                user_entry_item.downloaded,
                                                user_entry_item.key,
                                                user_entry_item.uploaded,
                                                user_entry_item.updated,
                                                structure.column_id,
                                                structure.column_completed,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                structure.column_updated
                                            )
                                        }
                                        false => {
                                            format!(
                                                "INSERT INTO `{}` (`{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}`) VALUES ('{}', {}, {}, {}, '{}', {}, {}) ON CONFLICT (`{}`) DO UPDATE SET `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`, `{}`=excluded.`{}`",
                                                structure.table_name,
                                                structure.column_id,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                user_entry_item.user_uuid.clone().unwrap(),
                                                user_entry_item.completed,
                                                user_entry_item.active,
                                                user_entry_item.downloaded,
                                                user_entry_item.key,
                                                user_entry_item.uploaded,
                                                user_entry_item.updated,
                                                structure.column_id,
                                                structure.column_completed,
                                                structure.column_completed,
                                                structure.column_active,
                                                structure.column_active,
                                                structure.column_downloaded,
                                                structure.column_downloaded,
                                                structure.column_key,
                                                structure.column_key,
                                                structure.column_uploaded,
                                                structure.column_uploaded,
                                                structure.column_updated,
                                                structure.column_updated
                                            )
                                        }
                                    }
                                }
                            }
                        }
                        false => {
                            match tracker.config.deref().clone().database_structure.users.id_uuid {
                                true => {
                                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                                        true => {
                                            format!(
                                                "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={}, `{}`={}, `{}`=X'{}', `{}`={}, `{}`={} WHERE `{}`=X'{}'",
                                                structure.table_name,
                                                structure.column_completed,
                                                user_entry_item.completed,
                                                structure.column_active,
                                                user_entry_item.active,
                                                structure.column_downloaded,
                                                user_entry_item.downloaded,
                                                structure.column_key,
                                                user_entry_item.key,
                                                structure.column_uploaded,
                                                user_entry_item.uploaded,
                                                structure.column_updated,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                user_entry_item.user_uuid.clone().unwrap(),
                                            )
                                        }
                                        false => {
                                            format!(
                                                "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={}, `{}`={}, `{}`='{}', `{}`={}, `{}`={} WHERE `{}`='{}'",
                                                structure.table_name,
                                                structure.column_completed,
                                                user_entry_item.completed,
                                                structure.column_active,
                                                user_entry_item.active,
                                                structure.column_downloaded,
                                                user_entry_item.downloaded,
                                                structure.column_key,
                                                user_entry_item.key,
                                                structure.column_uploaded,
                                                user_entry_item.uploaded,
                                                structure.column_updated,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                user_entry_item.user_uuid.clone().unwrap(),
                                            )
                                        }
                                    }
                                }
                                false => {
                                    match tracker.config.deref().clone().database_structure.users.bin_type_key {
                                        true => {
                                            format!(
                                                "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={}, `{}`={}, `{}`=X'{}', `{}`={}, `{}`={} WHERE `{}`=X'{}'",
                                                structure.table_name,
                                                structure.column_completed,
                                                user_entry_item.completed,
                                                structure.column_active,
                                                user_entry_item.active,
                                                structure.column_downloaded,
                                                user_entry_item.downloaded,
                                                structure.column_key,
                                                user_entry_item.key,
                                                structure.column_uploaded,
                                                user_entry_item.uploaded,
                                                structure.column_updated,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                user_entry_item.user_id.unwrap(),
                                            )
                                        }
                                        false => {
                                            format!(
                                                "UPDATE OR IGNORE `{}` SET `{}`={}, `{}`={}, `{}`={}, `{}`='{}', `{}`={}, `{}`={} WHERE `{}`='{}'",
                                                structure.table_name,
                                                structure.column_completed,
                                                user_entry_item.completed,
                                                structure.column_active,
                                                user_entry_item.active,
                                                structure.column_downloaded,
                                                user_entry_item.downloaded,
                                                structure.column_key,
                                                user_entry_item.key,
                                                structure.column_uploaded,
                                                user_entry_item.uploaded,
                                                structure.column_updated,
                                                user_entry_item.updated,
                                                structure.column_uuid,
                                                user_entry_item.user_id.unwrap(),
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    };
                    match sqlx::query(string_format.as_str()).execute(&mut *users_transaction).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("[SQLite] Error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
            if (users_handled_entries as f64 / 1000f64).fract() == 0.0 || users.len() as u64 == users_handled_entries {
                info!("[SQLite] Handled {} users", users_handled_entries);
            }
        }
        info!("[SQLite] Handled {} users", users_handled_entries);
        self.commit(users_transaction).await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn reset_seeds_peers(&self, tracker: Arc<TorrentTracker>) -> Result<(), Error>
    {
        let mut reset_seeds_peers_transaction = self.pool.begin().await?;
        let structure = tracker.config.deref().clone().database_structure.clone().torrents;
        let string_format = format!(
            "UPDATE `{}` SET `{}`=0, `{}`=0",
            structure.table_name,
            structure.column_seeds,
            structure.column_peers
        );
        match sqlx::query(string_format.as_str()).execute(&mut *reset_seeds_peers_transaction).await {
            Ok(_) => {}
            Err(e) => {
                error!("[SQLite] Error: {}", e);
                return Err(e);
            }
        }
        let _ = self.commit(reset_seeds_peers_transaction).await;
        Ok(())
    }

    #[tracing::instrument(level = "debug")]
    pub async fn commit(&self, transaction: Transaction<'_, Sqlite>) -> Result<(), Error>
    {
        match transaction.commit().await {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                error!("[SQLite] Error: {}", e);
                Err(e)
            }
        }
    }
}