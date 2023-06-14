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
    io,
    mem::{transmute, MaybeUninit},
    task::Poll,
};
mod winprelude {
    pub use std::os::windows::prelude::*;
    pub use winapi::{
        shared::{
            minwindef::{BOOL, DWORD, LPVOID},
            ntdef::HANDLE,
        },
        um::handleapi::INVALID_HANDLE_VALUE,
    };
}
use winprelude::*;

use winapi::{
    shared::winerror::ERROR_PIPE_NOT_CONNECTED,
    um::{
        handleapi::{DuplicateHandle, INVALID_HANDLE_VALUE},
        processthreadsapi::GetCurrentProcess,
    },
};

/// Objects which own handles which can be shared with another processes.
///
/// On Windows, like with most other operating systems, handles belong to specific processes. You shouldn't just send the value of a handle to another process (with a named pipe, for example) and expect it to work on the other side. For this to work, you need [`DuplicateHandle`] – the Win32 API function which duplicates a handle into the handle table of the specified process (the reciever is referred to by its handle). This trait exposes the `DuplicateHandle` functionality in a safe manner. If the handle is *inheritable*, however, all child processes of a process inherit the handle and thus can use the same value safely without the need to share it. *All Windows handle objects created by this crate are inheritable.*
///
/// **Implemented for all types inside this crate which implement [`AsRawHandle`] and are supposed to be shared between processes.**
///
/// [`DuplicateHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle " "
/// [`AsRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html " "
pub trait ShareHandle: AsRawHandle {
    /// Duplicates the handle to make it accessible in the specified process (taken as a handle to that process) and returns the raw value of the handle which can then be sent via some form of IPC, typically named pipes. This is the only way to use any form of IPC other than named pipes to communicate between two processes which do not have a parent-child relationship or if the handle wasn't created as inheritable, therefore named pipes paired with this are a hard requirement in order to communicate between processes if one wasn't spawned by another.
    ///
    /// Backed by [`DuplicateHandle`]. Doesn't require unsafe code since `DuplicateHandle` never leads to undefined behavior if the `lpTargetHandle` parameter is a valid pointer, only creates an error.
    #[allow(clippy::not_unsafe_ptr_arg_deref)] // Handles are not pointers, they have handle checks
    fn share(&self, reciever: BorrowedHandle<'_>) -> io::Result<HANDLE> {
        let (success, new_handle) = unsafe {
            let mut new_handle = INVALID_HANDLE_VALUE;
            let success = DuplicateHandle(
                GetCurrentProcess(),
                self.as_raw_handle(),
                reciever.as_raw_handle(),
                &mut new_handle,
                0,
                1,
                0,
            );
            (success != 0, new_handle)
        };
        ok_or_ret_errno!(success => new_handle)
    }
}
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeReader {}
impl ShareHandle for unnamed_pipe::UnnamedPipeReader {}
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeWriter {}
impl ShareHandle for unnamed_pipe::UnnamedPipeWriter {}

#[inline(always)]
fn weaken_buf_init(buf: &mut [u8]) -> &mut [MaybeUninit<u8>] {
    unsafe {
        // SAFETY: types are layout-compatible, only difference
        // is a relaxation of the init guarantee.
        transmute(buf)
    }
}

fn is_eof_like(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::BrokenPipe || e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as _)
}

#[allow(unused)]
fn downgrade_poll_eof<T: Default>(r: Poll<io::Result<T>>) -> Poll<io::Result<T>> {
    r.map(downgrade_eof)
}
fn downgrade_eof<T: Default>(r: io::Result<T>) -> io::Result<T> {
    match r {
        Err(e) if is_eof_like(&e) => Ok(T::default()),
        els => els,
    }
}
