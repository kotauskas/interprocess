use {
    super::thunk_broken_pipe_to_eof,
    crate::os::windows::{
        imports::HANDLE, named_pipe::tokio::ByteReaderPipeStream as ReadHalfImpl,
    },
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

pub struct OwnedReadHalf {
    pub(super) inner: ReadHalfImpl,
}
impl OwnedReadHalf {
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
            ReadHalfImpl::from_raw_handle(handle)?
        };
        Ok(Self { inner })
    }
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.inner)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl AsyncRead for OwnedReadHalf {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let rslt = self.pinproj().poll_read(cx, buf);
        let thunked = thunk_broken_pipe_to_eof(ready!(rslt));
        Poll::Ready(thunked)
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
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
