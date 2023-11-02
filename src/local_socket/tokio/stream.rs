use super::super::ToLocalSocketName;
use std::io;

impmod! {local_socket::tokio,
    LocalSocketStream as LocalSocketStreamImpl,
    ReadHalf as ReadHalfImpl,
    WriteHalf as WriteHalfImpl,
}

/// A Tokio-based local socket byte stream, obtained eiter from [`LocalSocketListener`](super::LocalSocketListener) or
/// by connecting to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use futures::{
///     io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
///     try_join,
/// };
/// use interprocess::local_socket::{tokio::LocalSocketStream, NameTypeSupport};
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
/// // so that we could concurrently act on both.
/// let (reader, mut writer) = conn.split();
/// let mut reader = BufReader::new(reader);
///
/// // Allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the write operation as writing our whole string.
/// let write = writer.write_all(b"Hello from client!\n");
/// // Describe the read operation as reading until a newline into our buffer.
/// let read = reader.read_line(&mut buffer);
///
/// // Concurrently perform both operations.
/// try_join!(write, read)?;
///
/// // Close the connection a bit earlier than you'd think we would. Nice practice!
/// drop((reader, writer));
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
        LocalSocketStreamImpl::connect(name).await.map(Self::from)
    }
    /// Splits a stream into a read half and a write half, which can be used to read and write the stream concurrently
    /// from independently spawned tasks, entailing a memory allocation.
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
}
#[doc(hidden)]
impl From<LocalSocketStreamImpl> for LocalSocketStream {
    #[inline]
    fn from(inner: LocalSocketStreamImpl) -> Self {
        Self(inner)
    }
}

// TODO Tokio I/O by ref
multimacro! {
    LocalSocketStream,
    forward_rbv(LocalSocketStreamImpl, &),
    forward_futures_ref_rw,
    forward_as_handle,
    forward_try_from_handle(LocalSocketStreamImpl),
    forward_debug,
    derive_futures_mut_rw,
    derive_asraw,
}

/// A read half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`](super::LocalSocketStream).
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
pub struct ReadHalf(pub(super) ReadHalfImpl);
multimacro! {
    ReadHalf,
    forward_rbv(ReadHalfImpl, &),
    forward_futures_ref_read,
    forward_as_handle,
    forward_debug,
    derive_futures_mut_read,
    derive_asraw,
}
/// A write half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`](super::LocalSocketStream).
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
// TODO remove this GitHub link and others like it
pub struct WriteHalf(pub(super) WriteHalfImpl);
multimacro! {
    WriteHalf,
    forward_rbv(WriteHalfImpl, &),
    forward_futures_ref_write,
    forward_as_handle,
    forward_debug,
    derive_futures_mut_write,
    derive_asraw,
}
