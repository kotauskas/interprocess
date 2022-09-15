use {
    futures_io::AsyncWrite,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSlice},
        pin::Pin,
        task::{Context, Poll},
    },
};

#[cfg(feature = "tokio_support")]
impmod! {local_socket::tokio,
    OwnedWriteHalf as OwnedWriteHalfImpl
}
#[cfg(not(feature = "tokio_support"))]
struct OwnedWriteHalfImpl;

/// An owned write half of a Tokio-based local socket stream, obtained by splitting a [`LocalSocketStream`].
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
///
/// [`LocalSocketStream`]: struct.LocalSocketStream.html " "
pub struct OwnedWriteHalf {
    pub(super) inner: OwnedWriteHalfImpl,
}
impl OwnedWriteHalf {
    /// Retrieves the identifier of the process on the opposite end of the local socket connection.
    ///
    /// # Platform-specific behavior
    /// ## macOS and iOS
    /// Not supported by the OS, will always generate an error at runtime.
    pub fn peer_pid(&self) -> io::Result<u32> {
        self.inner.peer_pid()
    }
    fn pinproj(&mut self) -> Pin<&mut OwnedWriteHalfImpl> {
        Pin::new(&mut self.inner)
    }
}

impl AsyncWrite for OwnedWriteHalf {
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

impl Debug for OwnedWriteHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

// TODO can't do this on Unix
//impl_as_raw_handle!(OwnedWriteHalf);
