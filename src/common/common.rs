use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::io::Cursor;
use std::time::{Duration, SystemTime};
use async_std::future;
use byteorder::{BigEndian, ReadBytesExt};
use fern::colors::{Color, ColoredLevelConfig};
use log::info;
use tokio_shutdown::Shutdown;
use crate::common::structs::custom_error::CustomError;
use crate::config::structs::configuration::Configuration;

pub fn parse_query(query: Option<String>) -> Result<HashMap<String, Vec<Vec<u8>>>, CustomError> {
    let mut queries: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
    match query {
        None => {}
        Some(result) => {
            let split_raw_query: Vec<&str> = result.split('&').collect();
            for query_item in split_raw_query {
                if !query_item.is_empty() {
                    if query_item.contains('=') {
                        let key_name_raw = query_item.split('=').collect::<Vec<&str>>()[0];
                        let key_name = percent_encoding::percent_decode_str(key_name_raw).decode_utf8_lossy().to_lowercase();
                        if !key_name.is_empty() {
                            let value_data_raw = query_item.split('=').collect::<Vec<&str>>()[1];
                            let value_data = percent_encoding::percent_decode_str(value_data_raw).collect::<Vec<u8>>();
                            match queries.get(&key_name) {
                                None => {
                                    let query: Vec<Vec<u8>> = vec![value_data];
                                    let _ = queries.insert(key_name, query);
                                }
                                Some(result) => {
                                    let mut result_mut = result.clone();
                                    result_mut.push(value_data);
                                    let _ = queries.insert(key_name, result_mut);
                                }
                            }
                        }
                    } else {
                        let key_name = percent_encoding::percent_decode_str(query_item).decode_utf8_lossy().to_lowercase();
                        if !key_name.is_empty() {
                            match queries.get(&key_name) {
                                None => {
                                    let query: Vec<Vec<u8>> = vec![];
                                    let _ = queries.insert(key_name, query);
                                }
                                Some(result) => {
                                    let mut result_mut = result.clone();
                                    result_mut.push(vec![]);
                                    let _ = queries.insert(key_name, result.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(queries)
}

pub fn udp_check_host_and_port_used(bind_address: String) {
    if cfg!(target_os = "windows") {
        match std::net::UdpSocket::bind(&bind_address) {
            Ok(e) => e,
            Err(data) => {
                sentry::capture_error(&data);
                panic!("Unable to bind to {} ! Exiting...", &bind_address);
            }
        };
    }
}

pub(crate) fn bin2hex(data: &[u8; 20], f: &mut Formatter) -> fmt::Result {
    let mut chars = [0u8; 40];
    binascii::bin2hex(data, &mut chars).expect("failed to hexlify");
    write!(f, "{}", std::str::from_utf8(&chars).unwrap())
}

pub fn hex2bin(data: String) -> Result<[u8; 20], CustomError>
{
    match hex::decode(data) {
        Ok(hash_result) => { Ok(<[u8; 20]>::try_from(hash_result[0..20].as_ref()).unwrap()) }
        Err(data) => {
            sentry::capture_error(&data);
            Err(CustomError::new("error converting hex to bin"))
        }
    }
}

pub fn print_type<T>(_: &T) {
    println!("{:?}", std::any::type_name::<T>());
}

pub fn return_type<T>(_: &T) -> String {
    format!("{:?}", std::any::type_name::<T>())
}

pub fn equal_string_check(source: &String, check: &String) -> bool
{
    if *source.to_string() == format!("{:?}", check) {
        return true;
    }
    println!("Source: {}", source);
    println!("Check:  {:?}", check);
    false
}

pub fn setup_logging(config: &Configuration)
{
    let level = match config.log_level.as_str() {
        "off" => log::LevelFilter::Off,
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => {
            panic!("Unknown log level encountered: '{}'", config.log_level.as_str());
        }
    };

    let colors = ColoredLevelConfig::new()
        .trace(Color::Cyan)
        .debug(Color::Magenta)
        .info(Color::Green)
        .warn(Color::Yellow)
        .error(Color::Red);

    if let Err(_err) = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} [{:width$}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.9f"),
                colors.color(record.level()),
                record.target(),
                message,
                width = 5
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .apply()
    {
        panic!("Failed to initialize logging.")
    }
    info!("logging initialized.");
}

pub async fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH).unwrap()
        .as_secs()
}

pub async fn convert_int_to_bytes(number: &u64) -> Vec<u8> {
    let mut return_data: Vec<u8> = Vec::new();
    for i in 1..8 {
        if number < &256u64.pow(i) {
            let start: usize = 16usize - i as usize;
            return_data.extend(number.to_be_bytes()[start..8].iter());
            return return_data;
        }
    }
    return_data
}

pub async fn convert_bytes_to_int(array: &Vec<u8>) -> u64 {
    let mut array_fixed: Vec<u8> = Vec::new();
    let size = 8 - array.len();
    array_fixed.resize(size, 0);
    array_fixed.extend(array);
    let mut rdr = Cursor::new(array_fixed);
    rdr.read_u64::<BigEndian>().unwrap()
}

pub async fn shutdown_waiting(timeout: Duration, shutdown_handler: Shutdown) -> bool
{
    match future::timeout(timeout, shutdown_handler.handle()).await {
        Ok(_) => {
            true
        }
        Err(_) => {
            false
        }
    }
}