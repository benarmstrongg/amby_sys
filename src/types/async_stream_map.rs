use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, RwLock};

pub type AsyncStreamMap = Arc<RwLock<HashMap<String, TcpStream>>>;
