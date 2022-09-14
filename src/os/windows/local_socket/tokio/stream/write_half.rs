use {
    crate::os::windows::{
        imports::HANDLE, named_pipe::tokio::ByteWriterPipeStream as WriteHalfImpl,
    },
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

pub struct OwnedWriteHalf {
    pub(super) inner: WriteHalfImpl,
}
impl OwnedWriteHalf {
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    // TODO use this
    pub unsafe fn _from_raw_handle(handle: HANDLE) -> io::Result<Self> {
        let inner = unsafe {
            // SAFETY: as per safety contract
            WriteHalfImpl::from_raw_handle(handle)?
        };
        Ok(Self { inner })
    }
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
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
    // Those two do nothing
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.pinproj().poll_flush(cx)
    }
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
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
