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
        ffi::{c_void, OsStr},
        fmt::{self, Debug, Formatter},
        future::Future,
        io,
        os::windows::io::AsRawHandle,
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    },
    tokio::time::{sleep, Instant, Sleep},
};

pub struct LocalSocketStream {
    pub(super) inner: PipeStream,
}
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = ConnectFuture::new(name.inner()).await?;
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

pub struct ConnectFuture<'a> {
    name: &'a OsStr,
    timer: Sleep,
}
impl<'a> ConnectFuture<'a> {
    const IDLE_TIME: Duration = Duration::from_millis(1);
    fn new(name: &'a OsStr) -> Self {
        Self {
            name,
            timer: sleep(Duration::new(0, 0)), // FIXME use Duration::ZERO
        }
    }
    fn reset_timer(self: Pin<&mut Self>) {
        self.pinproj_timer().reset(Instant::now() + Self::IDLE_TIME);
    }
    fn pinproj_timer(self: Pin<&mut Self>) -> Pin<&mut Sleep> {
        unsafe {
            // SAFETY: requires self to be pinned
            Pin::new_unchecked(&mut self.get_unchecked_mut().timer)
        }
    }
}
impl Future for ConnectFuture<'_> {
    type Output = io::Result<PipeStream>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match PipeStream::connect(self.as_ref().name) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                ready!(self.as_mut().pinproj_timer().poll(cx));
                self.as_mut().reset_timer();
                Poll::Pending
            }
            not_waiting => Poll::Ready(not_waiting),
        }
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
