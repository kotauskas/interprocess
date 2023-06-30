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

pub struct OwnedReadHalf(pub(super) ReadHalfImpl);
impl OwnedReadHalf {
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl AsyncRead for OwnedReadHalf {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
}
impl Debug for OwnedReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::OwnedWriteHalf").field(&self.0).finish()
    }
}
forward_as_handle!(OwnedReadHalf);
