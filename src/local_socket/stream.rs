use super::ToLocalSocketName;
use std::io;

impmod! {local_socket,
    LocalSocketStream as LocalSocketStreamImpl,
    ReadHalf as ReadHalfImpl,
    WriteHalf as WriteHalfImpl,
}

/// A local socket byte stream, obtained eiter from [`LocalSocketListener`](super::LocalSocketListener) or by connecting
/// to an existing local socket.
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
pub struct LocalSocketStream(pub(super) LocalSocketStreamImpl);
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self(LocalSocketStreamImpl::connect(name)?))
    }
    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, reading and writing will immediately return with the
    /// [`WouldBlock`](io::ErrorKind::WouldBlock) error in situations when they would normally block for an uncontrolled
    /// amount of time. The specific situations are:
    /// - When reading is attempted and there is no new data available;
    /// - When writing is attempted and the buffer is full due to the other side not yet having read previously sent
    /// data.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    /// Splits a stream into a read half and a write half, which can be used to read and write the stream concurrently
    /// from independently spawned tasks, entailing a memory allocation.
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
    // TODO reunite
}
multimacro! {
    LocalSocketStream,
    forward_rbv(LocalSocketStreamImpl, &),
    forward_sync_ref_rw,
    forward_asinto_handle,
    forward_debug,
    forward_try_from_handle(LocalSocketStreamImpl),
    derive_sync_mut_rw,
    derive_asintoraw,
}

/// A read half of a local socket stream, obtained by splitting a [`LocalSocketStream`](super::LocalSocketStream).
// TODO example
// TODO rename to RecvHalf
pub struct ReadHalf(pub(super) ReadHalfImpl);

multimacro! {
    ReadHalf,
    forward_rbv(ReadHalfImpl, &),
    forward_sync_ref_read,
    forward_as_handle,
    forward_debug,
    derive_sync_mut_read,
    derive_asraw,
}

/// A write half of a local socket stream, obtained by splitting a [`LocalSocketStream`](super::LocalSocketStream).
// TODO rename to SendHalf
pub struct WriteHalf(pub(super) WriteHalfImpl);

multimacro! {
    WriteHalf,
    forward_rbv(WriteHalfImpl, &),
    forward_sync_ref_write,
    forward_as_handle,
    forward_debug,
    derive_sync_mut_write,
    derive_asraw,
}

/// [`ReuniteError`](crate::error::ReuniteError) for sync local socket streams.
pub type ReuniteError = crate::error::ReuniteError<ReadHalf, WriteHalf>;
/// Result type for [`LocalSocketStream::reunite()`].
pub type ReuniteResult = Result<LocalSocketStream, ReuniteError>;
