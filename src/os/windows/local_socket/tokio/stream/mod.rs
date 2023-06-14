mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;
// TODO reunite

use crate::{
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{pipe_mode, tokio::DuplexPipeStream},
};
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    io,
    os::windows::prelude::*,
    pin::Pin,
    task::{Context, Poll},
};

type StreamImpl = DuplexPipeStream<pipe_mode::Bytes>;

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) StreamImpl);
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = DuplexPipeStream::connect(name.inner()).await?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.0.is_server() {
            true => self.0.client_process_id(),
            false => self.0.server_process_id(),
        }
    }
    #[inline]
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (r, w) = self.0.split();
        (OwnedReadHalf(r), OwnedWriteHalf(w))
    }
    pub fn reunite(rh: OwnedReadHalf, wh: OwnedWriteHalf) -> io::Result<Self> {
        match rh.0.reunite(wh.0) {
            Ok(inner) => Ok(Self(inner)),
            Err(_) => todo!(),
        }
    }
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut StreamImpl> {
        Pin::new(&mut self.0)
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
forward_as_handle!(LocalSocketStream);
impl TryFrom<OwnedHandle> for LocalSocketStream {
    type Error = (OwnedHandle, io::Error);

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        StreamImpl::try_from(handle)
            .map(Self)
            .map_err(|e| (e.handle, e.io_error))
    }
}
