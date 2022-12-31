use super::super::imports::*;
use std::{ffi::OsStr, io};

pub(crate) fn _connect(path: &OsStr, read: bool, write: bool) -> io::Result<TokioNPClient> {
    let result = TokioNPClientOptions::new()
        .read(read)
        .write(write)
        .open(path);
    match result {
        Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
            Err(io::ErrorKind::WouldBlock.into())
        }
        els => els,
    }
}
// TODO connect with wait
