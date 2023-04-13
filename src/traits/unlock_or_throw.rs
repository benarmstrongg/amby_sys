use log::error;
use std::process;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait UnlockOrThrow<T> {
    fn read_or_throw(&self) -> RwLockReadGuard<'_, T>;
    fn write_or_throw(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> UnlockOrThrow<T> for RwLock<T> {
    fn read_or_throw(&self) -> RwLockReadGuard<'_, T> {
        match self.read() {
            Ok(guard) => guard,
            Err(err) => {
                error!("Protocol stream map read lock poisned. Error: {err}");
                process::exit(1);
            }
        }
    }

    fn write_or_throw(&self) -> RwLockWriteGuard<'_, T> {
        match self.write() {
            Ok(guard) => guard,
            Err(err) => {
                error!("Protocol stream map write lock poisned. Error: {err}");
                process::exit(1);
            }
        }
    }
}
