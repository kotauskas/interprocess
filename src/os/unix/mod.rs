//! Unix-specific functionality for various interprocess communication primitives, as well as Unix-specific ones.
//!
//! ## FIFO files
//! This type of interprocess communication similar to unnamed pipes in that they are unidirectional byte channels which behave like files. The difference is that FIFO files are actual (pseudo)files on the filesystem and thus can be accessed by unrelated applications (one doesn't need to be spawned by another).
//!
//! FIFO files are available on all supported systems.
//!
//! ## Unix domain sockets
//! Those are sockets used specifically for local IPC. They support bidirectional connections, identification by file path or inside the abstract Linux socket namespace, optional preservation of message boundaries (`SOCK_DGRAM` UDP-like interface) and transferring file descriptor ownership.
//!
//! Unix domain sockets are not available on ARM Newlib, but are supported on all other Unix-like systems.

#![cfg_attr(not(unix), allow(warnings))]

mod imports;

pub mod fifo_file;
#[cfg(any(doc, feature = "signals"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "signals")))]
pub mod signal;

#[cfg(any(doc, uds_supported))]
pub mod udsocket;

#[cfg(unix)]
pub(crate) mod local_socket;
#[cfg(unix)]
pub(crate) mod unnamed_pipe;

use imports::*;
use std::{io, marker::PhantomData, mem::ManuallyDrop};

#[cfg(unix)]
pub(crate) struct FdOps(pub c_int, PhantomData<*mut ()>);
#[cfg(unix)]
impl FdOps {
    pub fn new(fd: c_int) -> Self {
        Self(fd, PhantomData)
    }
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let (success, num_bytes_read) = unsafe {
            let length_to_read = buf.len() as size_t;
            let size_or_err =
                libc::read(self.as_raw_fd(), buf.as_mut_ptr() as *mut _, length_to_read);
            if size_or_err >= 0 {
                (true, size_or_err as usize)
            } else {
                (false, 0)
            }
        };
        if success {
            Ok(num_bytes_read)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let (success, num_bytes_written) = unsafe {
            let length_to_write = buf.len() as size_t;
            let size_or_err =
                libc::write(self.as_raw_fd(), buf.as_ptr() as *const _, length_to_write);
            if size_or_err >= 0 {
                (true, size_or_err as usize)
            } else {
                (false, 0)
            }
        };
        if success {
            Ok(num_bytes_written)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn flush(&self) -> io::Result<()> {
        let success = unsafe { libc::fsync(self.as_raw_fd()) >= 0 };
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
#[cfg(unix)]
impl AsRawFd for FdOps {
    fn as_raw_fd(&self) -> c_int {
        self.0
    }
}
#[cfg(unix)]
impl IntoRawFd for FdOps {
    fn into_raw_fd(self) -> c_int {
        let self_ = ManuallyDrop::new(self);
        self_.as_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for FdOps {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self::new(fd)
    }
}
#[cfg(unix)]
impl Drop for FdOps {
    fn drop(&mut self) {
        unsafe { close_fd(self.0) };
    }
}
#[cfg(unix)]
unsafe impl Send for FdOps {}
#[cfg(unix)]
unsafe impl Sync for FdOps {}

unsafe fn close_fd(fd: i32) {
    let success = unsafe {
        let mut success = true;
        // If the close() call fails, the loop starts and keeps retrying until either the error
        // value isn't Interrupted (in which case the assertion fails) or the close operation
        // properly fails with a non-Interrupted error type. Why does Unix even have this
        // idiotic error type?
        while libc::close(fd) != 0 {
            if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                // An actual close error happened — return early now
                success = false;
                break;
            }
        }
        success
    };
    debug_assert!(success);
}
/// Captures [`io::Error::last_os_error()`] and closes the file descriptor.
unsafe fn handle_fd_error(fd: i32) -> io::Error {
    let e = io::Error::last_os_error();
    unsafe { close_fd(fd) };
    e
}
unsafe fn close_by_error(socket: i32) -> impl FnOnce(io::Error) -> io::Error {
    move |e| {
        unsafe { close_fd(socket) };
        e
    }
}
