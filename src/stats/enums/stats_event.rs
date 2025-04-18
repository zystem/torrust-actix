use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum StatsEvent {
    Torrents,
    TorrentsUpdates,
    Users,
    UsersUpdates,
    TimestampSave,
    TimestampTimeout,
    TimestampConsole,
    TimestampKeysTimeout,
    Seeds,
    Peers,
    Completed,
    WhitelistEnabled,
    Whitelist,
    WhitelistUpdates,
    BlacklistEnabled,
    Blacklist,
    BlacklistUpdates,
    Key,
    KeyUpdates,
    Tcp4NotFound,
    Tcp4Failure,
    Tcp4ConnectionsHandled,
    Tcp4ApiHandled,
    Tcp4AnnouncesHandled,
    Tcp4ScrapesHandled,
    Tcp6NotFound,
    Tcp6Failure,
    Tcp6ConnectionsHandled,
    Tcp6ApiHandled,
    Tcp6AnnouncesHandled,
    Tcp6ScrapesHandled,
    Udp4BadRequest,
    Udp4InvalidRequest,
    Udp4ConnectionsHandled,
    Udp4AnnouncesHandled,
    Udp4ScrapesHandled,
    Udp6BadRequest,
    Udp6InvalidRequest,
    Udp6ConnectionsHandled,
    Udp6AnnouncesHandled,
    Udp6ScrapesHandled
}