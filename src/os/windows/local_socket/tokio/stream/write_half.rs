use {
    crate::os::windows::named_pipe::{pipe_mode, tokio::SendHalf},
    futures_io::AsyncWrite,
    std::{
        fmt::{self, Debug, Formatter},
        io,
        pin::Pin,
        task::{Context, Poll},
    },
};

type WriteHalfImpl = SendHalf<pipe_mode::Bytes>;

pub struct OwnedWriteHalf(pub(super) WriteHalfImpl);
impl OwnedWriteHalf {
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
        Pin::new(&mut self.0)
    }
}
impl AsyncWrite for OwnedWriteHalf {
    #[inline]
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write(cx, buf)
    }
    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_flush(cx)
    }
    #[inline]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_close(cx)
    }
}

impl Debug for OwnedWriteHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::OwnedWriteHalf").field(&self.0).finish()
    }
}
forward_as_handle!(OwnedWriteHalf);
