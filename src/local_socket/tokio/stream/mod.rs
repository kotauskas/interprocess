mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;

use {
    super::super::ToLocalSocketName,
    futures_io::{AsyncRead, AsyncWrite},
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSlice, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

#[cfg(feature = "tokio_support")]
impmod! {local_socket::tokio,
    LocalSocketStream as LocalSocketStreamImpl
}
#[cfg(not(feature = "tokio_support"))]
struct LocalSocketStreamImpl;

/// A Tokio-based local socket byte stream, obtained eiter from [`LocalSocketListener`] or by connecting to an existing local socket.
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
pub struct LocalSocketStream {
    pub(super) inner: LocalSocketStreamImpl,
}
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self {
            inner: LocalSocketStreamImpl::connect(name).await?,
        })
    }
    /// Splits a stream into a read half and a write half, which can be used to read and write the stream concurrently.
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (r, w) = self.inner.into_split();
        (OwnedReadHalf { inner: r }, OwnedWriteHalf { inner: w })
    }
    /// Retrieves the identifier of the process on the opposite end of the local socket connection.
    ///
    /// # Platform-specific behavior
    /// ## macOS and iOS
    /// Not supported by the OS, will always generate an error at runtime.
    pub fn peer_pid(&self) -> io::Result<u32> {
        self.inner.peer_pid()
    }
    fn pinproj(&mut self) -> Pin<&mut LocalSocketStreamImpl> {
        Pin::new(&mut self.inner)
    }
}

impl AsyncRead for LocalSocketStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read_vectored(cx, bufs)
    }
}
impl AsyncWrite for LocalSocketStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write(cx, buf)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write_vectored(cx, bufs)
    }
    // Those don't do anything
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_flush(cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_close(cx)
    }
}

impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl_as_raw_handle!(LocalSocketStream);
