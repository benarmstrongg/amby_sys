use std::{net::TcpListener, process};

use log::{error, info};

pub trait TcpListenOrThrow {
    fn listen_or_throw(address: &str, name: &str) -> TcpListener;
}

impl TcpListenOrThrow for TcpListener {
    fn listen_or_throw(address: &str, name: &str) -> TcpListener {
        match TcpListener::bind(address) {
            Ok(tcp_listener) => {
                info!("TCP {name} server running at {address}",);
                tcp_listener
            }
            Err(err) => {
                error!("Error initializing {name} server. Error: {err}");
                process::exit(1);
            }
        }
    }
}
