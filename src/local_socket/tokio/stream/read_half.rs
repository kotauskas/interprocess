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
    ReadHalf as ReadHalfImpl
}

/// A read half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`](super::LocalSocketStream).
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}
impl AsyncRead for ReadHalf {
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
impl Debug for ReadHalf {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

forward_as_handle!(ReadHalf);
derive_asraw!(ReadHalf);
