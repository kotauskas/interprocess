use {
    crate::os::windows::named_pipe::{pipe_mode, tokio::RecvHalf},
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io,
        pin::Pin,
        task::{Context, Poll},
    },
};

type ReadHalfImpl = RecvHalf<pipe_mode::Bytes>;

pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}

impl AsyncRead for ReadHalf {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
}
impl Debug for ReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::WriteHalf").field(&self.0).finish()
    }
}
forward_as_handle!(ReadHalf);
