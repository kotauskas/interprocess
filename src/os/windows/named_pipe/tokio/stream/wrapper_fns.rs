use std::{ffi::OsStr, io};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient as TokioNPClient};
use winapi::shared::winerror::ERROR_PIPE_BUSY;

pub(crate) fn _connect(path: &OsStr, read: bool, write: bool) -> io::Result<TokioNPClient> {
    let result = ClientOptions::new().read(read).write(write).open(path);
    match result {
        Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => Err(io::ErrorKind::WouldBlock.into()),
        els => els,
    }
}
// TODO connect with wait
