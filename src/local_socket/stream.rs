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

/// A local socket byte stream, obtained eiter from [`LocalSocketListener`](super::LocalSocketListener) or by connecting to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// use interprocess::local_socket::{LocalSocketStream, NameTypeSupport};
/// use std::io::{prelude::*, BufReader};
///
/// // Pick a name. There isn't a helper function for this, mostly because it's largely unnecessary:
/// // in Rust, `match` is your concise, readable and expressive decision making construct.
/// let name = {
///     // This scoping trick allows us to nicely contain the import inside the `match`, so that if
///     // any imports of variants named `Both` happen down the line, they won't collide with the
///     // enum we're working with here. Maybe someone should make a macro for this.
///     use NameTypeSupport::*;
///     match NameTypeSupport::query() {
///         OnlyPaths => "/tmp/example.sock",
///         OnlyNamespaced | Both => "@example.sock",
///     }
/// };
///
/// // Preemptively allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let conn = LocalSocketStream::connect(name)?;
/// // Wrap it into a buffered reader right away so that we could read a single line out of it.
/// let mut conn = BufReader::new(conn);
///
/// // Write our message into the stream. This will finish either when the whole message has been
/// // writen or if a write operation returns an error. (`.get_mut()` is to get the writer,
/// // `BufReader` doesn't implement a pass-through `Write`.)
/// conn.get_mut().write_all(b"Hello from client!\n")?;
///
/// // We now employ the buffer we allocated prior and read a single line, interpreting a newline
/// // character as an end-of-file (because local sockets cannot be portably shut down), verifying
/// // validity of UTF-8 on the fly.
/// conn.read_line(&mut buffer)?;
///
/// // Print out the result, getting the newline for free!
/// print!("Server answered: {buffer}");
/// # std::io::Result::<()>::Ok(())
/// ```
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
forward_handle!(LocalSocketStream, inner);
derive_raw!(LocalSocketStream);
