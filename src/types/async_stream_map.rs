use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, RwLock};

use amby::Name;

pub type AsyncStreamMap = Arc<RwLock<HashMap<Name, TcpStream>>>;
