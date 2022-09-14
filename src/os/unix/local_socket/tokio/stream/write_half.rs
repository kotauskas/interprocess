use {
    crate::os::unix::udsocket::tokio::OwnedWriteHalf as OwnedWriteHalfImpl,
    futures_io::AsyncWrite,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSlice},
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct OwnedWriteHalf {
    pub(super) inner: OwnedWriteHalfImpl,
}
impl OwnedWriteHalf {
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
    fn pinproj(&mut self) -> Pin<&mut OwnedWriteHalfImpl> {
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
impl Debug for OwnedWriteHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::OwnedWriteHalf")
            .field(&self.inner)
            .finish()
    }
}
// TODO as_raw_fd
