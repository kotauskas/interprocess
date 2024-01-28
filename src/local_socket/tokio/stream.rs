use super::super::ToLocalSocketName;
use std::io;

impmod! {local_socket::tokio,
    LocalSocketStream as LocalSocketStreamImpl,
    RecvHalf as RecvHalfImpl,
    SendHalf as SendHalfImpl,
}

/// A Tokio-based local socket byte stream, obtained eiter from
/// [`LocalSocketListener`](super::LocalSocketListener) or by connecting to an existing local
/// socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::local_socket::{tokio::LocalSocketStream, NameTypeSupport};
/// use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, try_join};
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
/// // Await this here since we can't do a whole lot without a connection.
/// let conn = LocalSocketStream::connect(name).await?;
///
/// // This consumes our connection and splits it into two halves,
/// // so that we can concurrently use both.
/// let (recver, mut sender) = conn.split();
/// let mut recver = BufReader::new(recver);
///
/// // Allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the send operation as writing our whole string.
/// let send = sender.write_all(b"Hello from client!\n");
/// // Describe the receive operation as receiving until a newline into our buffer.
/// let recv = recver.read_line(&mut buffer);
///
/// // Concurrently perform both operations.
/// try_join!(send, recv)?;
///
/// // Close the connection a bit earlier than you'd think we would. Nice practice!
/// drop((recver, sender));
///
/// // Display the results when we're done!
/// println!("Server answered: {}", buffer.trim());
/// # Ok(()) }
/// ```
pub struct LocalSocketStream(pub(super) LocalSocketStreamImpl);
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    #[inline]
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        LocalSocketStreamImpl::connect(name.to_local_socket_name()?)
            .await
            .map(Self::from)
    }
    /// Splits a stream into a receive half and a send half, which can be used to receive data from
    /// and send data to the stream concurrently from independently spawned tasks, entailing a
    /// memory allocation.
    #[inline]
    pub fn split(self) -> (RecvHalf, SendHalf) {
        let (r, w) = self.0.split();
        (RecvHalf(r), SendHalf(w))
    }
}
#[doc(hidden)]
impl From<LocalSocketStreamImpl> for LocalSocketStream {
    #[inline]
    fn from(inner: LocalSocketStreamImpl) -> Self {
        Self(inner)
    }
}

multimacro! {
    LocalSocketStream,
    pinproj_for_unpin(LocalSocketStreamImpl),
    forward_rbv(LocalSocketStreamImpl, &),
    forward_tokio_rw,
    forward_tokio_ref_rw,
    forward_as_handle,
    forward_try_from_handle(LocalSocketStreamImpl),
    forward_debug,
    derive_asraw,
}

/// A receive half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`].
///
/// # Examples
// TODO
pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
    RecvHalf,
    pinproj_for_unpin(RecvHalfImpl),
    forward_rbv(RecvHalfImpl, &),
    forward_tokio_read,
    forward_tokio_ref_read,
    forward_as_handle,
    forward_debug,
    derive_asraw,
}
/// A send half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`].
///
/// # Examples
// TODO
pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
    SendHalf,
    pinproj_for_unpin(SendHalfImpl),
    forward_rbv(SendHalfImpl, &),
    forward_tokio_write,
    forward_tokio_ref_write,
    forward_as_handle,
    forward_debug,
    derive_asraw,
}
