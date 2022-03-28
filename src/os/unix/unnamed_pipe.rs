use super::FdOps;
use crate::{
    unnamed_pipe::{UnnamedPipeReader as PubReader, UnnamedPipeWriter as PubWriter},
    Sealed,
};
use libc::c_int;
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, Read, Write},
    mem::ManuallyDrop,
    os::unix::io::{AsRawFd, FromRawFd, IntoRawFd},
};

pub(crate) fn pipe() -> io::Result<(PubWriter, PubReader)> {
    let (success, fds) = unsafe {
        let mut fds: [c_int; 2] = [0; 2];
        let result = libc::pipe(fds.as_mut_ptr());
        (result == 0, fds)
    };
    if success {
        unsafe {
            // SAFETY: we just created both of those file descriptors, which means that neither of
            // them can be in use elsewhere.
            let reader = PubReader {
                inner: UnnamedPipeReader::from_raw_fd(fds[0]),
            };
            let writer = PubWriter {
                inner: UnnamedPipeWriter::from_raw_fd(fds[1]),
            };
            Ok((writer, reader))
        }
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) struct UnnamedPipeReader(FdOps);
// Please, for the love of Unix gods, don't ever try to implement this for &UnnamedPipeReader,
// reading a pipe concurrently is UB and UnnamedPipeReader is Send and Sync. If you do, the
// universe will collapse immediately.
impl Read for UnnamedPipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}
impl Sealed for UnnamedPipeReader {}
impl AsRawFd for UnnamedPipeReader {
    fn as_raw_fd(&self) -> c_int {
        self.0.as_raw_fd()
    }
}
impl IntoRawFd for UnnamedPipeReader {
    fn into_raw_fd(self) -> c_int {
        let self_ = ManuallyDrop::new(self);
        self_.as_raw_fd()
    }
}
impl FromRawFd for UnnamedPipeReader {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self(unsafe {
            // SAFETY: guaranteed by safety contract
            FdOps::from_raw_fd(fd)
        })
    }
}
impl Debug for UnnamedPipeReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeReader")
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}

pub(crate) struct UnnamedPipeWriter(FdOps);
impl Write for UnnamedPipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl Sealed for UnnamedPipeWriter {}
impl AsRawFd for UnnamedPipeWriter {
    #[cfg(unix)]
    fn as_raw_fd(&self) -> c_int {
        self.0.as_raw_fd()
    }
}
impl IntoRawFd for UnnamedPipeWriter {
    #[cfg(unix)]
    fn into_raw_fd(self) -> c_int {
        self.0.into_raw_fd()
    }
}
impl FromRawFd for UnnamedPipeWriter {
    #[cfg(unix)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self(FdOps::new(fd))
    }
}
impl Debug for UnnamedPipeWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeWriter")
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}
