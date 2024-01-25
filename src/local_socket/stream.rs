use super::ToLocalSocketName;
use std::io;

impmod! {local_socket,
    LocalSocketStream as LocalSocketStreamImpl,
    RecvHalf as RecvHalfImpl,
    SendHalf as SendHalfImpl,
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
/// // Preemptively allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let conn = LocalSocketStream::connect(name)?;
/// // Wrap it into a buffered reader right away so that we could receive a single line out of it.
/// let mut conn = BufReader::new(conn);
///
/// // Send our message into the stream. This will finish either when the whole message has been
/// // sent or if a send operation returns an error. (`.get_mut()` is to get the sender,
/// // `BufReader` doesn't implement pass-through `Write`.)
/// conn.get_mut().write_all(b"Hello from client!\n")?;
///
/// // We now employ the buffer we allocated prior and receive a single line, interpreting a newline
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
        LocalSocketStreamImpl::connect(name.to_local_socket_name()?).map(Self)
    }
    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, receiving and sending immediately returns with the
    /// [`WouldBlock`](io::ErrorKind::WouldBlock) error in situations when they would normally block
    /// for an uncontrolled amount of time. The specific situations are:
    /// - When receiving is attempted and there is no new data available;
    /// - When sending is attempted and the buffer is full due to the other side not yet having
    ///   received previously sent data.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    /// Splits a stream into a receive half and a send half, which can be used to receive from and
    /// send to the stream concurrently from different threads, entailing a memory allocation.
    #[inline]
    pub fn split(self) -> (RecvHalf, SendHalf) {
        let (r, w) = self.0.split();
        (RecvHalf(r), SendHalf(w))
    }
    /// Attempts to reunite a receive half with a send half to yield the original stream back,
    /// returning both halves as an error if they belong to different streams (or when using
    /// this method on streams that haven't been split to begin with).
    #[inline]
    pub fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult {
        LocalSocketStreamImpl::reunite(rh.0, sh.0)
            .map(Self)
            .map_err(|crate::error::ReuniteError { rh, sh }| ReuniteError {
                rh: RecvHalf(rh),
                sh: SendHalf(sh),
            })
    }
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

/// A receive half of a local socket stream, obtained by splitting a [`LocalSocketStream`].
// TODO example
pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
    RecvHalf,
    forward_rbv(RecvHalfImpl, &),
    forward_sync_ref_read,
    forward_as_handle,
    forward_debug,
    derive_sync_mut_read,
    derive_asraw,
}

/// A send half of a local socket stream, obtained by splitting a [`LocalSocketStream`].
pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
    SendHalf,
    forward_rbv(SendHalfImpl, &),
    forward_sync_ref_write,
    forward_as_handle,
    forward_debug,
    derive_sync_mut_write,
    derive_asraw,
}

/// [`ReuniteError`](crate::error::ReuniteError) for sync local socket streams.
pub type ReuniteError = crate::error::ReuniteError<RecvHalf, SendHalf>;
/// Result type for [`LocalSocketStream::reunite()`].
pub type ReuniteResult = Result<LocalSocketStream, ReuniteError>;
