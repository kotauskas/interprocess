use {
    crate::os::windows::named_pipe::{pipe_mode, tokio::SendHalf},
    futures_io::AsyncWrite,
    std::{
        ffi::c_void,
        fmt::{self, Debug, Formatter},
        io,
        os::windows::io::AsRawHandle,
        pin::Pin,
        task::{Context, Poll},
    },
};

type WriteHalfImpl = SendHalf<pipe_mode::Bytes>;

pub struct OwnedWriteHalf {
    pub(super) inner: WriteHalfImpl,
}
impl OwnedWriteHalf {
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
        Pin::new(&mut self.inner)
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
        f.debug_struct("local_socket::OwnedWriteHalf")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for OwnedWriteHalf {
    #[inline]
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
