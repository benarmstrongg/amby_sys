use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;

use amby::Name;

pub trait WriteToAllStreams {
    fn write_to_all_streams<T: Into<Vec<u8>>>(&self, data: T) -> std::io::Result<()>;
}

impl WriteToAllStreams for HashMap<Name, TcpStream> {
    fn write_to_all_streams<T: Into<Vec<u8>>>(&self, data: T) -> std::io::Result<()> {
        let map_ref = self;
        let bytes: Vec<u8> = data.into();
        for mut stream in map_ref.values() {
            stream.write_all(&bytes)?;
        }
        Ok(())
    }
}
