use {
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

#[cfg(feature = "tokio_support")]
impmod! {local_socket::tokio,
    OwnedReadHalf as OwnedReadHalfImpl
}
#[cfg(not(feature = "tokio_support"))]
struct OwnedReadHalfImpl;

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
    pub fn peer_pid(&self) -> io::Result<u32> {
        self.inner.peer_pid()
    }
    fn pinproj(&mut self) -> Pin<&mut OwnedReadHalfImpl> {
        Pin::new(&mut self.inner)
    }
}

impl AsyncRead for OwnedReadHalf {
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

impl Debug for OwnedReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

// TODO can't do this on Unix
//impl_as_raw_handle!(OwnedReadHalf);
