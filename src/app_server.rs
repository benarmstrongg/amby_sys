use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::{process, thread};

use amby::{AppMetadata, ReadAll, Response, ToBytesVec};
use log::{error, info};

use crate::traits::UnlockOrThrow;
use crate::types::AsyncStreamMap;
use crate::util::{retry, RETRY_LIMIT};

pub fn start_app_server(app_stream_map_clone: AsyncStreamMap) {
    thread::spawn(move || {
        let tcp_listener = match TcpListener::bind(format!("127.0.0.1:4000")) {
            Ok(tcp_listener) => {
                info!("App server running at 127.0.0.1:4000");
                tcp_listener
            }
            Err(err) => {
                error!("Error initializing app server. Error: {err}");
                process::exit(1);
            }
        };

        for incoming_connection in tcp_listener.incoming() {
            let tcp_stream = match incoming_connection {
                Ok(conn) => conn,
                Err(err) => {
                    error!("Error registering incoming app tcp connection. Error: {err}");
                    break;
                }
            };

            let app_stream_map_lock = Arc::clone(&app_stream_map_clone);
            thread::spawn(move || {
                register_app_client(&tcp_stream, &app_stream_map_lock);
            });
        }
    });
}

fn register_app_client(mut app_stream: &TcpStream, app_stream_map_lock: &AsyncStreamMap) {
    let app_name = match retry(RETRY_LIMIT, || {
        let mut app_stream = app_stream;
        match app_stream.read_all() {
            Ok(bytes) => match AppMetadata::try_from(bytes.to_owned()) {
                Ok(metadata) => Some(metadata),
                Err(err) => {
                    error!("Error parsing app metadata. Error: {err}");
                    None
                }
            },
            Err(err) => {
                error!("Error reading metadata from app stream. Error: {err}");
                info!("Retrying read app stream.");
                None
            }
        }
    }) {
        Some(metadata) => {
            let mut app_stream_map = app_stream_map_lock.write_or_throw();
            app_stream_map.insert(metadata.name.clone(), app_stream.try_clone().unwrap());
            info!("Registered app {}", &metadata.name);
            metadata.name
        }
        None => {
            error!("Error reading app metadata. Closing app stream");
            return;
        }
    };
    info!(
        "Received metadata for app {}. Sending success response",
        &app_name
    );
    match app_stream.write_all(&Response::Success(vec![]).to_bytes()) {
        Ok(()) => {
            info!("Registered app client {}", &app_name);
        }
        Err(err) => {
            error!(
                "Failed to send success response to protocol client; protocol was not registered. Error: {err}"
            );
        }
    };
}
