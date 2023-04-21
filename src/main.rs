use log::{error, info};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

mod app_server;
mod protocol_server;
mod traits;
mod types;
mod util;
use app_server::start_app_server;
use protocol_server::start_protocol_server;
use types::AsyncStreamMap;

fn main() {
    match simple_logger::init() {
        Ok(_) => info!("Logger initialized"),
        Err(err) => error!("Error initializing logger. Error: {err}"),
    };

    let protocol_stream_map_ref: AsyncStreamMap = Arc::new(RwLock::new(HashMap::new()));
    let app_stream_map_ref: AsyncStreamMap = Arc::new(RwLock::new(HashMap::new()));
    let event_stream_map_ref: AsyncStreamMap = Arc::new(RwLock::new(HashMap::new()));

    info!("Registering protocol server");
    start_protocol_server(
        Arc::clone(&protocol_stream_map_ref),
        Arc::clone(&app_stream_map_ref),
    );

    info!("Registering app server");
    start_app_server(
        Arc::clone(&app_stream_map_ref),
        Arc::clone(&event_stream_map_ref),
    );

    loop {}
}
