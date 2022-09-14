use {
    crate::os::unix::udsocket::tokio::OwnedReadHalf as OwnedReadHalfImpl,
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct OwnedReadHalf {
    pub(super) inner: OwnedReadHalfImpl,
}
impl OwnedReadHalf {
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
    fn pinproj(&mut self) -> Pin<&mut OwnedReadHalfImpl> {
        Pin::new(&mut self.inner)
    }
}
impl AsyncRead for OwnedReadHalf {
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
impl Debug for OwnedReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::OwnedReadHalf")
            .field(&self.inner)
            .finish()
    }
}
// TODO as_raw_fd
