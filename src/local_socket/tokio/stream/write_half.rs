use {
    futures_io::AsyncWrite,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSlice},
        pin::Pin,
        task::{Context, Poll},
    },
};

impmod! {local_socket::tokio,
    WriteHalf as WriteHalfImpl
}

/// A write half of a Tokio-based local socket stream, obtained by splitting a [`LocalSocketStream`].
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
///
/// [`LocalSocketStream`]: struct.LocalSocketStream.html " "
pub struct WriteHalf(pub(super) WriteHalfImpl);
impl WriteHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
        Pin::new(&mut self.0)
    }
}

impl AsyncWrite for WriteHalf {
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

impl Debug for WriteHalf {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

forward_as_handle!(WriteHalf);
derive_asraw!(WriteHalf);
