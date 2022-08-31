#[cfg(uds_supported)]
use super::c_wrappers;
use super::{OwnedReadHalf, ReuniteError, UdStream};
use crate::os::unix::imports::*;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};

/// Borrowed write half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct BorrowedWriteHalf<'a>(pub(super) TokioUdStreamWriteHalf<'a>);

impl<'a> BorrowedWriteHalf<'a> {
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
    /// Shuts down the write half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_raw_fd().as_ref(), Shutdown::Write)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the write half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_raw_fd(&self) -> c_int {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_raw_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamWriteHalf<'a>> {
        Pin::new(&mut self.get_mut().0)
    }

    tokio_wrapper_conversion_methods!(tokio_norawfd TokioUdStreamWriteHalf<'a>);
}

#[cfg(feature = "tokio_support")]
impl TokioAsyncWrite for BorrowedWriteHalf<'_> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.pinproject().poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_shutdown(cx)
    }
}
#[cfg(feature = "tokio_support")]
impl FuturesAsyncWrite for BorrowedWriteHalf<'_> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.pinproject().poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
}

tokio_wrapper_trait_impls!(
    for BorrowedWriteHalf<'a>, tokio_norawfd_lt 'a TokioUdStreamWriteHalf<'a>);

/// Owned write half of a [`UdStream`](super::UdStream), created by [`.into_split()`](super::UdStream::into_split).
#[derive(Debug)]
pub struct OwnedWriteHalf(pub(super) TokioUdStreamOwnedWriteHalf);
impl OwnedWriteHalf {
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the two halves originated from the same call to [`.into_split()`](UdStream::into_split).
    pub fn reunite_with(self, read: OwnedReadHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(read, self)
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

    /// Shuts down the write half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_raw_fd().as_ref(), Shutdown::Write)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the write half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_raw_fd(&self) -> c_int {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_raw_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamOwnedWriteHalf> {
        Pin::new(&mut self.get_mut().0)
    }

    tokio_wrapper_conversion_methods!(tokio_norawfd TokioUdStreamOwnedWriteHalf);
}

#[cfg(feature = "tokio_support")]
impl TokioAsyncWrite for OwnedWriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
}
#[cfg(feature = "tokio_support")]
impl FuturesAsyncWrite for OwnedWriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.pinproject().poll_flush(cx)
    }
    /// Finishes immediately. See the `.shutdown()` method.
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
}

tokio_wrapper_trait_impls!(
    for OwnedWriteHalf, tokio_norawfd TokioUdStreamOwnedWriteHalf);
