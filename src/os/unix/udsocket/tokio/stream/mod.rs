use crate::os::unix::udsocket::{c_wrappers, ToUdSocketPath, UdSocketPath, UdStream as SyncUdStream};
use crate::os::unix::unixprelude::*;
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    error::Error,
    fmt::{self, Formatter},
    io,
    net::Shutdown,
    os::unix::net::UnixStream as StdUdStream,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf as TokioReadBuf},
    net::{unix::ReuniteError as TokioReuniteError, UnixStream as TokioUdStream},
};

mod connect_future;
mod read_half;
mod write_half;
use connect_future::*;
pub use {read_half::*, write_half::*};

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`](super::UdStreamListener) or by connecting to an existing server.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::os::unix::udsocket::tokio::*;
/// use tokio::{
///     io::{AsyncReadExt, AsyncWriteExt},
///     try_join,
/// };
///
/// // Await this here since we can't do a whole lot without a connection.
/// let mut conn = UdStream::connect("/tmp/example.sock").await?;
///
/// // This takes an exclusive borrow of our connection and splits it into two
/// // halves, so that we could concurrently act on both. Take care not to use
/// // the .split() method from the futures crate's AsyncReadExt.
/// let (mut reader, mut writer) = conn.split();
///
/// // Allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the write operation as writing our whole string, waiting for
/// // that to complete, and then shutting down the write half, which sends
/// // an EOF to the other end to help it determine where the message ends.
/// let write = async {
///     writer.write_all(b"Hello from client!\n").await?;
///     writer.shutdown()?;
///     Ok(())
/// };
///
/// // Describe the read operation as reading until EOF into our big buffer.
/// let read = reader.read_to_string(&mut buffer);
///
/// // Concurrently perform both operations: write-and-send-EOF and read.
/// try_join!(write, read)?;
///
/// // Close the connection a bit earlier than you'd think we would. Nice practice!
/// drop(conn);
///
/// // Display the results when we're done!
/// println!("Server answered: {}", buffer.trim());
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct UdStream(TokioUdStream);
impl UdStream {
    /// Connects to a Unix domain socket server at the specified path.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    pub async fn connect(path: impl ToUdSocketPath<'_>) -> io::Result<Self> {
        let path = path.to_socket_path()?;
        Self::_connect(&path).await
    }
    async fn _connect(path: &UdSocketPath<'_>) -> io::Result<Self> {
        let stream = ConnectFuture { path }.await?;
        Self::from_sync(stream)
    }

    /// Borrows a stream into a read half and a write half, which can be used to read and write the stream concurrently.
    ///
    /// This method is more efficient than [`.into_split()`](Self::into_split), but the halves cannot be moved into independently spawned tasks.
    pub fn split(&mut self) -> (BorrowedReadHalf<'_>, BorrowedWriteHalf<'_>) {
        let (read_tok, write_tok) = self.0.split();
        (BorrowedReadHalf(read_tok), BorrowedWriteHalf(write_tok))
    }
    /// Splits a stream into a read half and a write half, which can be used to read and write the stream concurrently.
    ///
    /// Unlike [`.split()`](Self::split), the owned halves can be moved to separate tasks, which comes at the cost of a heap allocation.
    ///
    /// Dropping either half will shut it down. This is equivalent to calling [`.shutdown()`](Self::shutdown) on the stream with the corresponding argument.
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (read_tok, write_tok) = self.0.into_split();
        (OwnedReadHalf(read_tok), OwnedWriteHalf(write_tok))
    }
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the two halves originated from the same call to [`.into_split()`](Self::into_split).
    pub fn reunite(read: OwnedReadHalf, write: OwnedWriteHalf) -> Result<Self, ReuniteError> {
        let (read_tok, write_tok) = (read.0, write.0);
        let stream_tok = read_tok.reunite(write_tok)?;
        Ok(Self::from_tokio(stream_tok))
    }

    /// Shuts down the read, write, or both halves of the stream. See [`Shutdown`].
    ///
    /// Attempting to call this method with the same `how` argument multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        c_wrappers::shutdown(self.0.as_fd(), how)
    }
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(uds_peerucred)]
    #[cfg_attr( // uds_peerucred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox",
            target_os = "haiku"
        )))
    )]
    pub fn get_peer_credentials(&self) -> io::Result<libc::ucred> {
        c_wrappers::get_peer_ucred(self.0.as_fd())
    }
    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStream> {
        Pin::new(&mut self.get_mut().0)
    }
    tokio_wrapper_conversion_methods!(
        sync SyncUdStream,
        std StdUdStream,
        tokio TokioUdStream);
}
tokio_wrapper_trait_impls!(
    for UdStream,
    sync SyncUdStream,
    std StdUdStream,
    tokio TokioUdStream);
derive_asraw!(unix: UdStream);

impl TokioAsyncRead for UdStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
impl AsyncRead for UdStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl TokioAsyncWrite for UdStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        self.pinproject().poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_shutdown(cx)
    }
}
impl AsyncWrite for UdStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        self.pinproject().poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown(Shutdown::Both)?;
        Poll::Ready(Ok(()))
    }
}

/// Error indicating that a read half and a write half were not from the same stream, and thus could not be reunited.
#[derive(Debug)]
pub struct ReuniteError(pub OwnedReadHalf, pub OwnedWriteHalf);
impl Error for ReuniteError {}
impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tried to reunite halves of different streams")
    }
}
impl From<TokioReuniteError> for ReuniteError {
    fn from(TokioReuniteError(read, write): TokioReuniteError) -> Self {
        let read = OwnedReadHalf::from_tokio(read);
        let write = OwnedWriteHalf::from_tokio(write);
        Self(read, write)
    }
}
impl From<ReuniteError> for TokioReuniteError {
    fn from(ReuniteError(read, write): ReuniteError) -> Self {
        let read = read.into_tokio();
        let write = write.into_tokio();
        Self(read, write)
    }
}
