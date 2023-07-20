use {
    crate::os::unix::udsocket::tokio::ReadHalf as ReadHalfImpl,
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}
impl AsyncRead for ReadHalf {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> std::task::Poll<io::Result<usize>> {
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::ReadHalf").field(&self.0).finish()
    }
}
forward_as_handle!(ReadHalf);
