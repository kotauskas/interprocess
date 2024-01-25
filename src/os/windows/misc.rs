pub(super) mod winprelude {
    pub(crate) use super::AsRawHandleExt;
    pub(crate) use std::os::windows::prelude::*;
    pub(crate) use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
}

use std::{
    io::{self, ErrorKind::BrokenPipe},
    task::Poll,
};
use winprelude::*;

pub(crate) trait AsRawHandleExt: AsRawHandle {
    #[inline(always)]
    fn as_int_handle(&self) -> HANDLE {
        self.as_raw_handle() as HANDLE
    }
}
impl<T: AsRawHandle + ?Sized> AsRawHandleExt for T {}

pub(super) fn decode_eof<T>(r: io::Result<T>) -> io::Result<T> {
    use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
    match r {
        Err(e) if e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as _) => Err(io::Error::from(BrokenPipe)),
        els => els,
    }
}
pub(super) fn downgrade_eof<T: Default>(r: io::Result<T>) -> io::Result<T> {
    match decode_eof(r) {
        Err(e) if e.kind() == BrokenPipe => Ok(T::default()),
        els => els,
    }
}
#[allow(unused)]
pub(super) fn downgrade_poll_eof<T: Default>(r: Poll<io::Result<T>>) -> Poll<io::Result<T>> {
    r.map(downgrade_eof)
}
