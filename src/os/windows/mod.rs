//! Windows-specific functionality for various interprocess communication primitives, as well as Windows-specific ones.
#![cfg_attr(not(windows), allow(warnings))]

pub mod named_pipe;
pub mod unnamed_pipe;
// TODO mailslots
//pub mod mailslot;
pub(crate) mod local_socket;

mod file_handle;
pub(crate) use file_handle::*;

use std::{
    io::{self, ErrorKind::BrokenPipe},
    task::Poll,
};
mod winprelude {
    pub(crate) use super::AsRawHandleExt;
    pub(crate) use std::os::windows::prelude::*;
    pub(crate) use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
}
use winprelude::*;

mod c_wrappers;

/// Objects which own handles which can be shared with another processes.
///
/// On Windows, like with most other operating systems, handles belong to specific processes. You shouldn't just send
/// the value of a handle to another process (with a named pipe, for example) and expect it to work on the other side.
/// For this to work, you need [`DuplicateHandle`](windows_sys::Win32::Foundation::DuplicateHandle) – the Win32 API function
/// which duplicates a handle into the handle table of the specified process (the receiver is referred to by its
/// handle). This trait exposes the `DuplicateHandle` functionality in a safe manner.
///
/// Note that the resulting handle is expected not to be inheritable. It is a logic error to have the output of
/// `.share()` to be inheritable, but it is not UB.
///
/// **Implemented for all types inside this crate which implement [`AsHandle`] and are supposed to be shared between
/// processes.**
pub trait ShareHandle: AsHandle {
    /// Duplicates the handle to make it accessible in the specified process (taken as a handle to that process) and
    /// returns the raw value of the handle which can then be sent via some form of IPC, typically named pipes. This is
    /// the only way to use any form of IPC other than named pipes to communicate between two processes which do not
    /// have a parent-child relationship or if the handle wasn't created as inheritable.
    ///
    /// Backed by [`DuplicateHandle`](windows_sys::Win32::Foundation::DuplicateHandle). Doesn't require unsafe code since
    /// `DuplicateHandle` never leads to undefined behavior if the `lpTargetHandle` parameter is a valid pointer, only
    /// creates an error.
    #[allow(clippy::not_unsafe_ptr_arg_deref)] // Handles are not pointers, they have handle checks
    fn share(&self, receiver: BorrowedHandle<'_>) -> io::Result<HANDLE> {
        c_wrappers::duplicate_handle_to_foreign(self.as_handle(), receiver)
    }
}
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeReader {}
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeWriter {}

fn decode_eof<T>(r: io::Result<T>) -> io::Result<T> {
    use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
    match r {
        Err(e) if e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as _) => Err(io::Error::from(BrokenPipe)),
        els => els,
    }
}
fn downgrade_eof<T: Default>(r: io::Result<T>) -> io::Result<T> {
    match decode_eof(r) {
        Err(e) if e.kind() == BrokenPipe => Ok(T::default()),
        els => els,
    }
}
#[allow(unused)]
fn downgrade_poll_eof<T: Default>(r: Poll<io::Result<T>>) -> Poll<io::Result<T>> {
    r.map(downgrade_eof)
}

pub(crate) trait AsRawHandleExt: AsRawHandle {
    #[inline(always)]
    fn as_int_handle(&self) -> HANDLE {
        self.as_raw_handle() as HANDLE
    }
}
impl<T: AsRawHandle + ?Sized> AsRawHandleExt for T {}
