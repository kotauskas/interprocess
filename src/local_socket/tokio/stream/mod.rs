mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;

use {
    super::super::ToLocalSocketName,
    futures_io::{AsyncRead, AsyncWrite},
    std::{
        io::{self, IoSlice, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

impmod! {local_socket::tokio,
    LocalSocketStream as LocalSocketStreamImpl
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
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut LocalSocketStreamImpl> {
        Pin::new(&mut self.0)
    }
}
#[doc(hidden)]
impl From<LocalSocketStreamImpl> for LocalSocketStream {
    #[inline]
    fn from(inner: LocalSocketStreamImpl) -> Self {
        Self(inner)
    }
}

// TODO I/O by ref

impl AsyncRead for LocalSocketStream {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
    #[inline]
    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read_vectored(cx, bufs)
    }
}
impl AsyncWrite for LocalSocketStream {
    #[inline]
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write(cx, buf)
    }
    #[inline]
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write_vectored(cx, bufs)
    }
    // Those don't do anything
    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_flush(cx)
    }
    #[inline]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_close(cx)
    }
}

multimacro! {
    LocalSocketStream,
    forward_as_handle,
    forward_try_from_handle(LocalSocketStreamImpl),
    forward_debug,
    derive_asraw,
}
