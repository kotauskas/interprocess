use {
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

impmod! {local_socket::tokio,
    OwnedReadHalf as OwnedReadHalfImpl
}

/// An owned read half of a Tokio-based local socket stream, obtained by splitting a [`LocalSocketStream`].
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
///
/// [`LocalSocketStream`]: struct.LocalSocketStream.html " "
pub struct OwnedReadHalf {
    pub(super) inner: OwnedReadHalfImpl,
}
impl OwnedReadHalf {
    /// Retrieves the identifier of the process on the opposite end of the local socket connection.
    ///
    /// # Platform-specific behavior
    /// ## macOS and iOS
    /// Not supported by the OS, will always generate an error at runtime.
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        self.inner.peer_pid()
    }
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut OwnedReadHalfImpl> {
        Pin::new(&mut self.inner)
    }
}

impl AsyncRead for OwnedReadHalf {
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

impl Debug for OwnedReadHalf {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

forward_as_handle!(OwnedReadHalf, inner);
derive_asraw!(OwnedReadHalf);
