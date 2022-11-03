use {
    super::ToLocalSocketName,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, prelude::*, IoSlice, IoSliceMut},
    },
};

impmod! {local_socket,
    LocalSocketStream as LocalSocketStreamImpl
}

/// A local socket byte stream, obtained eiter from [`LocalSocketListener`] or by connecting to an existing local socket.
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/local_socket/client.rs)
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
pub struct LocalSocketStream {
    pub(super) inner: LocalSocketStreamImpl,
}
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self {
            inner: LocalSocketStreamImpl::connect(name)?,
        })
    }
    /// Retrieves the identifier of the process on the opposite end of the local socket connection.
    ///
    /// # Platform-specific behavior
    /// ## macOS and iOS
    /// Not supported by the OS, will always generate an error at runtime.
    pub fn peer_pid(&self) -> io::Result<u32> {
        self.inner.peer_pid()
    }
    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, reading and writing will immediately return with the [`WouldBlock`] error in situations when they would normally block for an uncontrolled amount of time. The specific situations are:
    /// - When reading is attempted and there is no new data available;
    /// - When writing is attempted and the buffer is full due to the other side not yet having read previously sent data.
    ///
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}
// TODO panic on read-to-end and read-to-string
// TODO vectored I/O on Unix
impl Read for LocalSocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }
}
impl Write for LocalSocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}
impl_handle_manip!(LocalSocketStream);
