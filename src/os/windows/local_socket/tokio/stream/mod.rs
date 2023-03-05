mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;
// TODO reunite

use {
    crate::{
        local_socket::ToLocalSocketName,
        os::windows::named_pipe::{pipe_mode, tokio::DuplexPipeStream},
    },
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

type StreamImpl = DuplexPipeStream<pipe_mode::Bytes>;

pub struct LocalSocketStream {
    pub(super) inner: StreamImpl,
}
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = DuplexPipeStream::connect(name.inner()).await?;
        Ok(Self { inner })
    }
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    #[inline]
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (r, w) = self.inner.split();
        (OwnedReadHalf { inner: r }, OwnedWriteHalf { inner: w })
    }
    pub fn reunite(rh: OwnedReadHalf, wh: OwnedWriteHalf) -> io::Result<Self> {
        match rh.inner.reunite(wh.inner) {
            Ok(inner) => Ok(Self { inner }),
            Err(_) => todo!(),
        }
    }
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut StreamImpl> {
        Pin::new(&mut self.inner)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl AsyncRead for LocalSocketStream {
    #[inline]
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
}
impl AsyncWrite for LocalSocketStream {
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
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for LocalSocketStream {
    #[inline]
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
