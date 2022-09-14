mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;

use {
    super::super::local_socket_name_to_ud_socket_path,
    crate::{local_socket::ToLocalSocketName, os::unix::udsocket::tokio::UdStream},
    futures_io::{AsyncRead, AsyncWrite},
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSlice, IoSliceMut},
        os::unix::io::AsRawFd,
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct LocalSocketStream {
    pub(super) inner: UdStream,
}
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStream::connect(path).await?;
        Ok(Self { inner })
    }
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        let (r, w) = self.inner.into_split();
        (OwnedReadHalf { inner: r }, OwnedWriteHalf { inner: w })
    }
    pub fn peer_pid(&self) -> io::Result<u32> {
        #[cfg(uds_peercred)]
        {
            self.inner
                .get_peer_credentials()
                .map(|ucred| ucred.pid as u32)
        }
        #[cfg(not(uds_peercred))]
        {
            Err(io::Error::new(io::ErrorKind::Other, "not supported"))
        }
    }
    // TODO use those
    pub unsafe fn _from_raw_fd(fd: i32) -> io::Result<Self> {
        let inner = unsafe {
            // SAFETY: as per safety contract
            UdStream::from_raw_fd(fd)?
        };
        Ok(Self { inner })
    }
    pub fn _into_raw_fd(self) -> io::Result<i32> {
        self.inner.into_raw_fd()
    }
    fn pinproj(&mut self) -> Pin<&mut UdStream> {
        Pin::new(&mut self.inner)
    }
}
impl AsyncRead for LocalSocketStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        self.pinproj().poll_read(cx, buf)
    }
    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_read_vectored(cx, bufs)
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
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.pinproj().poll_write_vectored(cx, bufs)
    }

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
            .field("fd", &self.inner.as_raw_fd())
            .finish()
    }
}
impl AsRawFd for LocalSocketStream {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}
