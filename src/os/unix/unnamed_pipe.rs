use super::FdOps;
use crate::{
    unnamed_pipe::{UnnamedPipeReader as PubReader, UnnamedPipeWriter as PubWriter},
    Sealed,
};
use libc::c_int;
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, Read, Write},
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::io::{AsRawFd, FromRawFd},
    },
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
impl Read for &UnnamedPipeReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.0).read(buf)
    }
}
impl Read for UnnamedPipeReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (self as &Self).read(buf)
    }
}
impl Sealed for UnnamedPipeReader {}
impl AsFd for UnnamedPipeReader {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0 .0.as_fd()
    }
}
impl From<UnnamedPipeReader> for OwnedFd {
    #[inline]
    fn from(x: UnnamedPipeReader) -> Self {
        x.0 .0
    }
}
impl From<OwnedFd> for UnnamedPipeReader {
    #[inline]
    fn from(fd: OwnedFd) -> Self {
        Self(FdOps(fd))
    }
}
impl Debug for UnnamedPipeReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeReader")
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}
// TODO move from here to public API
derive_raw!(unix: UnnamedPipeReader);

pub(crate) struct UnnamedPipeWriter(FdOps);
impl Write for &UnnamedPipeWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.0).write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&self.0).flush()
    }
}
impl Write for UnnamedPipeWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self as &Self).write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (self as &Self).flush()
    }
}
impl Sealed for UnnamedPipeWriter {}
impl AsFd for UnnamedPipeWriter {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0 .0.as_fd()
    }
}
impl From<UnnamedPipeWriter> for OwnedFd {
    #[inline]
    fn from(x: UnnamedPipeWriter) -> Self {
        x.0 .0
    }
}
impl From<OwnedFd> for UnnamedPipeWriter {
    #[inline]
    fn from(fd: OwnedFd) -> Self {
        Self(FdOps(fd))
    }
}
impl Debug for UnnamedPipeWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeWriter")
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}
derive_raw!(unix: UnnamedPipeWriter);
