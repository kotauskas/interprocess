use super::{
    ancillary_io::sync::{read_in_terms_of_vectored, write_in_terms_of_vectored},
    ancwrap, c_wrappers,
    cmsg::{CmsgMut, CmsgRef},
    ReadAncillary, ReadAncillarySuccess, ToUdSocketPath, UdSocketPath, WriteAncillary,
};
use crate::{
    os::unix::{unixprelude::*, FdOps},
    TryClone,
};
use libc::{sockaddr_un, SOCK_STREAM};
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use to_method::To;

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`](super::UdStreamListener) or by
/// connecting to an existing server.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// use interprocess::os::unix::udsocket::UdStream;
/// use std::io::prelude::*;
///
/// let mut conn = UdStream::connect("/tmp/example1.sock")?;
/// conn.write_all(b"Hello from client!")?;
/// let mut string_buffer = String::new();
/// conn.read_to_string(&mut string_buffer)?;
/// println!("Server answered: {}", string_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
// TODO update with comments and stuff
#[derive(Debug)]
pub struct UdStream(FdOps);
impl UdStream {
    /// Connects to a Unix domain socket server at the specified path.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `socket`
    /// - `connect`
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_connect(path.to_socket_path()?, false)
    }
    #[cfg(feature = "tokio")]
    pub(crate) fn connect_nonblocking<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_connect(path.to_socket_path()?, true)
    }
    fn _connect(path: UdSocketPath<'_>, nonblocking: bool) -> io::Result<Self> {
        let addr = path.try_to::<sockaddr_un>()?;

        let fd = c_wrappers::create_uds(SOCK_STREAM, nonblocking)?;
        unsafe {
            // SAFETY: addr is well-constructed
            c_wrappers::connect(fd.0.as_fd(), &addr)?;
        }

        Ok(Self(fd))
    }
}

/// A list of used system calls is available.
impl Read for &UdStream {
    /// # System calls
    /// - `read`
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.0).read(buf)
    }
    /// # System calls
    /// - `readv`
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (&self.0).read_vectored(bufs)
    }
}
/// A list of used system calls is available.
impl Read for UdStream {
    /// # System calls
    /// - `read`
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
    /// # System calls
    /// - `readv`
    #[inline(always)]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (&*self).read_vectored(bufs)
    }
}

/// A list of used system calls is available.
impl<AB: CmsgMut + ?Sized> ReadAncillary<AB> for &UdStream {
    /// Implemented in terms of `read_ancillary_vectored`.
    ///
    /// # System calls
    /// - `recvmsg`
    #[inline]
    fn read_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB) -> io::Result<ReadAncillarySuccess> {
        read_in_terms_of_vectored(self, buf, abuf)
    }
    /// # System calls
    /// - `recvmsg`
    #[inline]
    fn read_ancillary_vectored(
        &mut self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> io::Result<ReadAncillarySuccess> {
        ancwrap::recvmsg(self.as_fd(), bufs, abuf, None)
    }
}
/// A list of used system calls is available.
impl<AB: CmsgMut + ?Sized> ReadAncillary<AB> for UdStream {
    /// Implemented in terms of `read_ancillary_vectored()`.
    ///
    /// # System calls
    /// - `recvmsg`
    #[inline(always)]
    fn read_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB) -> io::Result<ReadAncillarySuccess> {
        (&*self).read_ancillary(buf, abuf)
    }
    /// # System calls
    /// - `recvmsg`
    #[inline(always)]
    fn read_ancillary_vectored(
        &mut self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> io::Result<ReadAncillarySuccess> {
        (&*self).read_ancillary_vectored(bufs, abuf)
    }
}

/// A list of used system calls is available.
impl Write for &UdStream {
    /// # System calls
    /// - `write`
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.0).write(buf)
    }
    /// # System calls
    /// - `writev`
    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&self.0).write_vectored(bufs)
    }
    /// # System calls
    /// None performed.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        // You cannot flush a socket
        Ok(())
    }
}
/// A list of used system calls is available.
impl Write for UdStream {
    /// # System calls
    /// - `write`
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    /// # System calls
    /// - `writev`
    #[inline(always)]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }
    /// # System calls
    /// None performed.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        // You cannot flush a socket
        Ok(())
    }
}

/// A list of used system calls is available.
impl WriteAncillary for &UdStream {
    /// Implemented in terms of `write_ancillary_vectored()`.
    ///
    /// # System calls
    /// - `sendmsg`
    #[inline]
    fn write_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_>) -> io::Result<usize> {
        write_in_terms_of_vectored(self, buf, abuf)
    }
    /// # System calls
    /// - `sendmsg`
    #[inline]
    fn write_ancillary_vectored(&mut self, bufs: &[IoSlice<'_>], abuf: CmsgRef<'_>) -> io::Result<usize> {
        ancwrap::sendmsg(self.as_fd(), bufs, abuf)
    }
}
/// A list of used system calls is available.
impl WriteAncillary for UdStream {
    /// Implemented in terms of `write_ancillary_vectored()`.
    ///
    /// # System calls
    /// - `sendmsg`
    #[inline(always)]
    fn write_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_>) -> io::Result<usize> {
        (&*self).write_ancillary(buf, abuf)
    }
    /// # System calls
    /// - `sendmsg`
    #[inline(always)]
    fn write_ancillary_vectored(&mut self, bufs: &[IoSlice<'_>], abuf: CmsgRef<'_>) -> io::Result<usize> {
        (&*self).write_ancillary_vectored(bufs, abuf)
    }
}

impl TryClone for UdStream {
    fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
}

impl AsFd for UdStream {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0 .0.as_fd()
    }
}
impl From<UdStream> for OwnedFd {
    #[inline]
    fn from(x: UdStream) -> Self {
        x.0 .0
    }
}
impl From<OwnedFd> for UdStream {
    #[inline]
    fn from(fd: OwnedFd) -> Self {
        UdStream(FdOps(fd))
    }
}

derive_raw!(unix: UdStream);
