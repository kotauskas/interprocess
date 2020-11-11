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

pub mod fifo_file;
pub mod signal;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "emscripten",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "hermit",
    target_os = "redox",
    // For some unknown reason, Newlib only declares sockaddr_un on Xtensa, which is why we don't
    // support Ud-sockets on ARM
    all(target_env = "newlib", target_arch = "xtensa"),
    target_env = "uclibc",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",

    doc,
))]
pub mod udsocket;

#[cfg(unix)]
pub(crate) mod unnamed_pipe;
#[cfg(unix)]
pub(crate) mod local_socket;

#[cfg(unix)]
use libc::{
    c_int, size_t,
};
#[cfg(unix)]
use std::{
    io,
    os::unix::io::{AsRawFd, IntoRawFd, FromRawFd},
    mem,
};

#[cfg(unix)]
pub(crate) struct FdOps (pub(crate) c_int);
#[cfg(unix)]
impl FdOps {
    #[inline]
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let (success, num_bytes_read) = unsafe {
            let length_to_read = buf.len() as size_t;
            let size_or_err = libc::read(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut _,
                length_to_read,
            );
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
    #[inline]
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let (success, num_bytes_written) = unsafe {
            let length_to_write = buf.len() as size_t;
            let size_or_err = libc::write(
                self.as_raw_fd(),
                buf.as_ptr() as *const _,
                length_to_write,
            );
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
    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        let success = unsafe {
            libc::fsync(self.as_raw_fd()) >= 0
        };
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
#[cfg(unix)]
impl AsRawFd for FdOps {
    #[inline(always)]
    fn as_raw_fd(&self) -> c_int {
        self.0
    }
}
#[cfg(unix)]
impl IntoRawFd for FdOps {
    #[inline(always)]
    fn into_raw_fd(self) -> c_int {
        let fd = self.as_raw_fd();
        mem::forget(self);
        fd
    }
}
#[cfg(unix)]
impl FromRawFd for FdOps {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self(fd)
    }
}
#[cfg(unix)]
impl Drop for FdOps {
    #[inline]
    fn drop(&mut self) {
        debug_assert!(unsafe {
            let mut success = true;
            // If the close() call fails, the loop starts and keeps retrying until either the error
            // value isn't Interrupted (in which case the assertion fails) or the close operation
            // properly fails with a non-Interrupted error type. Why does Unix even have this
            // idiotic error type?
            while libc::close(self.0) != 0 {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    // An actual close error happened — return early now
                    success = false;
                    break;
                }
            }
            success
        });
    }
}