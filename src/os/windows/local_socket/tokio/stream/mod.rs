mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;
// TODO reunite

use {
    super::super::thunk_broken_pipe_to_eof,
    crate::{
        local_socket::ToLocalSocketName,
        os::windows::{imports::HANDLE, named_pipe::tokio::DuplexBytePipeStream as PipeStream},
    },
    futures_core::ready,
    futures_io::{AsyncRead, AsyncWrite},
    std::{
        ffi::c_void,
        fmt::{self, Debug, Formatter},
        io,
        os::windows::io::AsRawHandle,
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct LocalSocketStream {
    pub(super) inner: PipeStream,
}
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeStream::connect(name.inner())?;
        Ok(Self { inner })
    }
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (r, w) = self.inner.split();
        (OwnedReadHalf { inner: r }, OwnedWriteHalf { inner: w })
    }
    // TODO use this
    pub unsafe fn _from_raw_handle(handle: HANDLE) -> io::Result<Self> {
        let inner = unsafe {
            // SAFETY: as per safety contract
            PipeStream::from_raw_handle(handle)?
        };
        Ok(Self { inner })
    }
    fn pinproj(&mut self) -> Pin<&mut PipeStream> {
        Pin::new(&mut self.inner)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl AsyncRead for LocalSocketStream {
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
impl AsyncWrite for LocalSocketStream {
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
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for LocalSocketStream {
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
