use {
    crate::os::windows::named_pipe::{pipe_mode, tokio::RecvHalf},
    futures_core::ready,
    futures_io::AsyncRead,
    std::{
        ffi::c_void,
        fmt::{self, Debug, Formatter},
        io,
        os::windows::io::AsRawHandle,
        pin::Pin,
        task::{Context, Poll},
    },
};

type ReadHalfImpl = RecvHalf<pipe_mode::Bytes>;

pub struct OwnedReadHalf {
    pub(super) inner: ReadHalfImpl,
}
impl OwnedReadHalf {
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.inner)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl AsyncRead for OwnedReadHalf {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let rslt = self.pinproj().poll_read(cx, buf);
        Poll::Ready(ready!(rslt))
    }
}
impl Debug for OwnedReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("local_socket::OwnedWriteHalf")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for OwnedReadHalf {
    #[inline]
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
