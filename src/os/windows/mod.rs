//! Windows-specific functionality for various interprocess communication primitives, as well as Windows-specific ones.
#![cfg_attr(not(windows), allow(warnings))]

pub mod named_pipe;
#[cfg(any(doc, feature = "signals"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "signals")))]
pub mod signal;
pub mod unnamed_pipe;
// TODO mailslots
//pub mod mailslot;
#[cfg(windows)]
pub(crate) mod local_socket;

pub(crate) mod imports;
use imports::*;

use std::{io, mem::ManuallyDrop, ptr};

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
    fn share(&self, reciever: HANDLE) -> io::Result<HANDLE> {
        let (success, new_handle) = unsafe {
            let mut new_handle = INVALID_HANDLE_VALUE;
            let success = DuplicateHandle(
                GetCurrentProcess(),
                self.as_raw_handle(),
                reciever,
                &mut new_handle,
                0,
                1,
                0,
            );
            (success != 0, new_handle)
        };
        if success {
            Ok(new_handle)
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
#[cfg(windows)]
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeReader {}
#[cfg(windows)]
impl ShareHandle for unnamed_pipe::UnnamedPipeReader {}
#[cfg(windows)]
impl ShareHandle for crate::unnamed_pipe::UnnamedPipeWriter {}
#[cfg(windows)]
impl ShareHandle for unnamed_pipe::UnnamedPipeWriter {}

/// Newtype wrapper which defines file I/O operations on a `HANDLE` to a file.
#[repr(transparent)]
#[derive(Debug)]
pub(crate) struct FileHandleOps(pub(crate) HANDLE);
impl FileHandleOps {
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        debug_assert!(
            buf.len() <= DWORD::max_value() as usize,
            "buffer is bigger than maximum buffer size for ReadFile",
        );
        let (success, num_bytes_read) = unsafe {
            let mut num_bytes_read: DWORD = 0;
            let result = ReadFile(
                self.0,
                buf.as_mut_ptr() as *mut _,
                buf.len() as DWORD,
                &mut num_bytes_read as *mut _,
                ptr::null_mut(),
            );
            (result != 0, num_bytes_read as usize)
        };
        if success {
            Ok(num_bytes_read)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        debug_assert!(
            buf.len() <= DWORD::max_value() as usize,
            "buffer is bigger than maximum buffer size for WriteFile",
        );
        let (success, bytes_written) = unsafe {
            let mut number_of_bytes_written: DWORD = 0;
            let result = WriteFile(
                self.0,
                buf.as_ptr() as *mut _,
                buf.len() as DWORD,
                &mut number_of_bytes_written as *mut _,
                ptr::null_mut(),
            );
            (result != 0, number_of_bytes_written as usize)
        };
        if success {
            Ok(bytes_written)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn flush(&self) -> io::Result<()> {
        let success = unsafe { FlushFileBuffers(self.0) != 0 };
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
impl Drop for FileHandleOps {
    fn drop(&mut self) {
        let _success = unsafe { CloseHandle(self.0) != 0 };
        debug_assert!(
            _success,
            "failed to close file handle: {}",
            io::Error::last_os_error()
        );
    }
}
#[cfg(windows)]
impl AsRawHandle for FileHandleOps {
    fn as_raw_handle(&self) -> HANDLE {
        self.0
    }
}
#[cfg(windows)]
impl IntoRawHandle for FileHandleOps {
    fn into_raw_handle(self) -> HANDLE {
        let self_ = ManuallyDrop::new(self);
        self_.as_raw_handle()
    }
}
#[cfg(windows)]
impl FromRawHandle for FileHandleOps {
    unsafe fn from_raw_handle(op: HANDLE) -> Self {
        Self(op)
    }
}
unsafe impl Send for FileHandleOps {}
unsafe impl Sync for FileHandleOps {} // WriteFile and ReadFile are thread-safe, apparently
