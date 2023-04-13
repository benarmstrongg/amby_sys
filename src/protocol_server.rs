use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::{process, thread};

use amby::{ReadAll, ReadRequest, Request, Response, ToBytesVec, TryFromSlice, WriteRequest};
use log::{error, info};

use crate::traits::UnlockOrThrow;
use crate::types::AsyncStreamMap;
use crate::util::{retry, RETRY_LIMIT};

pub fn start_protocol_server(
    protocol_stream_map_clone: AsyncStreamMap,
    app_stream_map_clone: AsyncStreamMap,
) {
    thread::spawn(move || {
        let tcp_listener = match TcpListener::bind(format!("127.0.0.1:4001")) {
            Ok(tcp_listener) => tcp_listener,
            Err(err) => {
                error!("Error initializing protocol server. Error: {err}");
                process::exit(1);
            }
        };
        info!("Protocol server running at 127.0.0.1:4001");

        for incoming_connection in tcp_listener.incoming() {
            info!("Protocol client connection received. Registering protocol client");
            let tcp_stream = match incoming_connection {
                Ok(conn) => conn,
                Err(err) => {
                    error!("Error registering incoming protocol client connection. Error: {err}");
                    break;
                }
            };
            info!("Protocol client connected");
            let protocol_stream_map_lock = Arc::clone(&protocol_stream_map_clone);
            let app_stream_map_lock = Arc::clone(&app_stream_map_clone);
            thread::spawn(move || {
                let protocol_name =
                    match register_protocol_client(&tcp_stream, &protocol_stream_map_lock) {
                        Ok(name) => name,
                        Err(()) => return,
                    };
                loop {
                    let req = match pool_request(&tcp_stream, &protocol_stream_map_lock) {
                        Ok(req) => req,
                        Err(()) => continue,
                    };
                    info!("Protocol request received from client {}", &protocol_name);
                    let app_name = match &req {
                        Request::Read(ReadRequest { app_name, .. })
                        | Request::Write(WriteRequest { app_name, .. }) => app_name.clone(),
                    };
                    info!(
                        "Protocol request parsed. Sending request to app {}",
                        &app_name
                    );
                    let app_stream = match send_request(&app_name, req, &app_stream_map_lock) {
                        Ok(app_stream) => app_stream,
                        Err(()) => continue,
                    };
                    info!("Waiting for response from app {}", &app_name);
                    let res = match pool_response(&app_name, &app_stream) {
                        Ok(res) => res,
                        Err(()) => continue,
                    };
                    info!(
                        "Response received from app {}. Sending response to protocol {}",
                        &app_name, &protocol_name
                    );
                    send_response(&tcp_stream, res);
                }
            });
        }
    });
}

fn register_protocol_client(
    mut protocol_stream: &TcpStream,
    stream_map_lock: &AsyncStreamMap,
) -> Result<String, ()> {
    info!("Registering protocol client stream");
    let protocol_name = match protocol_stream.read_all() {
        Ok(bytes) => {
            let protocol_name = match String::try_from_slice(&bytes) {
                Ok(name) => name,
                Err(err) => {
                    error!("Error parsing string name from protocol stream. Error: {err}");
                    return Err(());
                }
            };
            let mut protocol_stream_map = stream_map_lock.write_or_throw();
            if !protocol_stream_map.contains_key(&protocol_name) {
                protocol_stream_map
                    .insert(protocol_name.clone(), protocol_stream.try_clone().unwrap());
                protocol_name
            } else {
                error!("Protocol with name {protocol_name} already exists. Failed to register protocol");
                return Err(());
            }
        }
        Err(err) => {
            error!("Error registering protocol stream. Error: {err}");
            return Err(());
        }
    };
    info!(
        "Received name {} for protocol. Sending success response",
        &protocol_name
    );
    match protocol_stream.write_all(&Response::Success(vec![]).to_bytes()) {
        Ok(()) => {
            info!("Registered protocol client {}", &protocol_name);
            Ok(protocol_name)
        }
        Err(err) => {
            error!(
                "Failed to send success response to protocol client; protocol was not registered. Error: {err}"
            );
            Err(())
        }
    }
}

fn pool_request(
    mut protocol_stream: &TcpStream,
    protocol_stream_map_lock: &AsyncStreamMap,
) -> Result<Request, ()> {
    match protocol_stream.read_all() {
        Ok(bytes) => match Request::try_from(bytes) {
            Ok(req) => match &req {
                Request::Read(ReadRequest { protocol_name, .. })
                | Request::Write(WriteRequest { protocol_name, .. }) => {
                    let protocol_stream_map = protocol_stream_map_lock.read_or_throw();
                    if !protocol_stream_map.contains_key(protocol_name) {
                        return Err(());
                    }
                    Ok(req)
                }
            },
            Err(err) => {
                error!("Error parsing protocol request; bad request. Error: {err}");
                return Err(());
            }
        },
        Err(err) => {
            error!("Error reading from protocol stream; waiting for next message. Error: {err}");
            return Err(());
        }
    }
}

fn send_request(
    app_name: &str,
    req: Request,
    app_stream_map_lock: &AsyncStreamMap,
) -> Result<TcpStream, ()> {
    match retry(RETRY_LIMIT, || {
        let req = req.clone();
        let app_stream_map = app_stream_map_lock.read_or_throw();
        let mut app_stream = match app_stream_map.get(app_name) {
            Some(stream) => stream.try_clone().unwrap(),
            None => {
                error!("No app stream found with id {app_name}. Request ignored.");
                return None;
            }
        };
        drop(app_stream_map);
        match app_stream.write_all(&req.to_bytes()) {
            Ok(()) => {
                info!("Successfully sent request to app {app_name}");
            }
            Err(err) => {
                error!("Failed to send request to app; retrying. Error: {err}");
                return None;
            }
        };
        info!("Sent protocol request to app {app_name}");
        Some(app_stream)
    }) {
        Some(app_stream) => Ok(app_stream),
        None => {
            error!("Failed to send request to app after {RETRY_LIMIT} tries. Aborting");
            Err(())
        }
    }
}

fn pool_response(app_name: &str, app_stream: &TcpStream) -> Result<Response, ()> {
    match retry(RETRY_LIMIT, move || {
        let mut app_stream = app_stream;
        match app_stream.read_all() {
            Ok(res) => Some(res),
            Err(err) => {
                error!("Error reading app stream response; retrying. Error: {err}");
                None
            }
        }
    }) {
        Some(res) => Ok(Response::from(res)),
        None => {
            error!(
                "Failed to read response from app {app_name} after {RETRY_LIMIT} tries. Aborting"
            );
            Err(())
        }
    }
}

fn send_response(protocol_stream: &TcpStream, res: Response) {
    match retry(RETRY_LIMIT, move || {
        let mut protocol_stream = protocol_stream;
        let res = res.clone();
        match protocol_stream.write_all(&res.to_bytes()) {
            Ok(()) => {
                info!("Wrote app response to protocol stream.");
                Some(())
            }
            Err(err) => {
                error!("Error writing app response to protocol stream; retrying. Error: {err}");
                None
            }
        }
    }) {
        Some(()) => {}
        None => {
            error!("Error writing app response to protocol stream. Will not retry.");
        }
    }
}
