#[cfg(uds_supported)]
use super::c_wrappers;
use super::{OwnedWriteHalf, ReuniteError, UdStream};
use crate::os::unix::imports::*;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};

/// Borrowed read half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct BorrowedReadHalf<'a>(pub(super) TokioUdStreamReadHalf<'a>);

impl<'a> BorrowedReadHalf<'a> {
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(any(doc, uds_peercred))]
    #[cfg_attr( // uds_peercred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox",
            target_os = "haiku"
        )))
    )]
    pub fn get_peer_credentials(&self) -> io::Result<ucred> {
        c_wrappers::get_peer_ucred(self.as_stream_raw_fd().as_ref())
    }
    /// Shuts down the read half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_raw_fd().as_ref(), Shutdown::Read)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the read half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_raw_fd(&self) -> c_int {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_raw_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamReadHalf<'a>> {
        Pin::new(&mut self.get_mut().0)
    }

    tokio_wrapper_conversion_methods!(tokio_norawfd TokioUdStreamReadHalf<'a>);
}

#[cfg(feature = "tokio_support")]
impl TokioAsyncRead for BorrowedReadHalf<'_> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
#[cfg(feature = "tokio_support")]
impl FuturesAsyncRead for BorrowedReadHalf<'_> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

tokio_wrapper_trait_impls!(
    for BorrowedReadHalf<'a>, tokio_norawfd_lt 'a TokioUdStreamReadHalf<'a>);

/// Owned read half of a [`UdStream`](super::UdStream), created by [`.into_split()`](super::UdStream::into_split).
#[derive(Debug)]
pub struct OwnedReadHalf(pub(super) TokioUdStreamOwnedReadHalf);
impl OwnedReadHalf {
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the two halves originated from the same call to [`.into_split()`](UdStream::into_split).
    pub fn reunite_with(self, write: OwnedWriteHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(self, write)
    }

    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(any(doc, uds_peercred))]
    #[cfg_attr( // uds_peercred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox",
            target_os = "haiku"
        )))
    )]
    pub fn get_peer_credentials(&self) -> io::Result<ucred> {
        c_wrappers::get_peer_ucred(self.as_stream_raw_fd().as_ref())
    }

    /// Shuts down the read half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_raw_fd().as_ref(), Shutdown::Read)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the read half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_raw_fd(&self) -> c_int {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_raw_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamOwnedReadHalf> {
        Pin::new(&mut self.get_mut().0)
    }

    tokio_wrapper_conversion_methods!(tokio_norawfd TokioUdStreamOwnedReadHalf);
}

#[cfg(feature = "tokio_support")]
impl TokioAsyncRead for OwnedReadHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
#[cfg(feature = "tokio_support")]
impl FuturesAsyncRead for OwnedReadHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

tokio_wrapper_trait_impls!(
    for OwnedReadHalf, tokio_norawfd TokioUdStreamOwnedReadHalf);
