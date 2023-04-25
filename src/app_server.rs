use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;

use amby::Name;
use amby::{AppMetadata, ReadAll, Response, ToBytes};
use log::{error, info};

use crate::traits::TcpListenOrThrow;
use crate::traits::UnlockOrThrow;
use crate::traits::WriteToAllStreams;
use crate::types::AsyncStreamMap;
use crate::util::{retry, RETRY_LIMIT};

pub fn start_app_server(
    app_stream_map_clone: AsyncStreamMap,
    event_stream_map_clone: AsyncStreamMap,
) {
    thread::spawn(move || {
        let app_tcp_listener = TcpListener::listen_or_throw("127.0.0.1:4000", "app");
        let event_tcp_listener = TcpListener::listen_or_throw("127.0.0.1:4002", "event");

        let event_stream_map_lock = Arc::clone(&event_stream_map_clone);
        thread::spawn(move || {
            let mut connection_count = 0;
            for incoming_connection in event_tcp_listener.incoming() {
                let event_stream = match incoming_connection {
                    Ok(conn) => conn,
                    Err(err) => {
                        error!("Error registering incoming event tcp connection. Error: {err}");
                        break;
                    }
                };
                let mut event_stream_map = event_stream_map_lock.write_or_throw();
                event_stream_map.insert(
                    Name::from_str_unchecked(&connection_count.to_string()),
                    event_stream,
                );
                connection_count += 1;
            }
        });

        for incoming_connection in app_tcp_listener.incoming() {
            let app_stream = match incoming_connection {
                Ok(conn) => conn,
                Err(err) => {
                    error!("Error registering incoming app tcp connection. Error: {err}");
                    break;
                }
            };

            let app_stream_map_lock = Arc::clone(&app_stream_map_clone);
            let event_stream_map_lock = Arc::clone(&event_stream_map_clone);
            thread::spawn(move || {
                let app_metadata = match register_app_client(&app_stream, &app_stream_map_lock) {
                    Ok(metadata) => metadata,
                    Err(()) => return,
                };
                let event_stream_map = event_stream_map_lock.read_or_throw();
                match event_stream_map.write_to_all_streams(app_metadata) {
                    Ok(()) => info!("App metadata sent to event streams"),
                    Err(err) => {
                        error!("Failed to send app metadata to protocol streams. Error {err}")
                    }
                };
            });
        }
    });
}

fn register_app_client(
    mut app_stream: &TcpStream,
    app_stream_map_lock: &AsyncStreamMap,
) -> Result<AppMetadata, ()> {
    let app_metadata = match retry(RETRY_LIMIT, || {
        let mut app_stream = app_stream;
        match app_stream.read_all() {
            Ok(bytes) => match AppMetadata::try_from(bytes) {
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
            metadata
        }
        None => {
            error!("Error reading app metadata. Closing app stream");
            return Err(());
        }
    };
    info!(
        "Received metadata for app {}. Sending success response",
        &app_metadata.name
    );
    match app_stream.write_all(&Response::Success(vec![]).to_bytes()) {
        Ok(()) => {
            info!("Registered app client {}", &app_metadata.name);
            Ok(app_metadata)
        }
        Err(err) => {
            error!(
                "Failed to send success response to protocol client; protocol was not registered. Error: {err}"
            );
            Err(())
        }
    }
}
