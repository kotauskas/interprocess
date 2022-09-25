#[cfg(uds_supported)]
use super::c_wrappers;
use {
    crate::os::unix::{
        imports::*,
        udsocket::{ToUdSocketPath, UdSocketPath, UdStream as SyncUdStream},
    },
    std::{
        convert::TryFrom,
        error::Error,
        fmt::{self, Formatter},
        io,
        net::Shutdown,
        pin::Pin,
        task::{Context, Poll},
    },
};

mod connect_future;
mod read_half;
mod write_half;
use connect_future::*;
pub use {read_half::*, write_half::*};

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`](super::UdStreamListener) or by connecting to an existing server.
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_udstream/client.rs)
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
        c_wrappers::shutdown(self.as_raw_fd().as_ref(), how)
    }
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(any(doc, uds_peercred))]
    #[cfg_attr( // uds_peercred template
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
    pub fn get_peer_credentials(&self) -> io::Result<ucred> {
        c_wrappers::get_peer_ucred(self.as_raw_fd().as_ref())
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

#[cfg(feature = "tokio_support")]
impl TokioAsyncRead for UdStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
#[cfg(feature = "tokio_support")]
impl FuturesAsyncRead for UdStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
#[cfg(feature = "tokio_support")]
impl TokioAsyncWrite for UdStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
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
#[cfg(feature = "tokio_support")]
impl FuturesAsyncWrite for UdStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
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
