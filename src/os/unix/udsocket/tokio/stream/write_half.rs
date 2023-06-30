use super::{c_wrappers, OwnedReadHalf, ReuniteError, UdStream};
use crate::os::unix::unixprelude::*;
use futures_io::AsyncWrite;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::AsyncWrite as TokioAsyncWrite,
    net::{
        unix::{OwnedWriteHalf as TokioUdStreamOwnedWriteHalf, WriteHalf as TokioUdStreamWriteHalf},
        UnixStream as TokioUdStream,
    },
};

/// Borrowed write half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct BorrowedWriteHalf<'a>(pub(super) TokioUdStreamWriteHalf<'a>);

impl<'a> BorrowedWriteHalf<'a> {
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn get_peer_credentials(&self) -> io::Result<libc::ucred> {
        c_wrappers::get_peer_ucred(self.as_stream_fd())
    }
    /// Shuts down the write half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_fd(), Shutdown::Write)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the write half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_fd(&self) -> BorrowedFd<'_> {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamWriteHalf<'a>> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncWrite for BorrowedWriteHalf<'_> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
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
impl AsyncWrite for BorrowedWriteHalf<'_> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
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
impl AsFd for BorrowedWriteHalf<'_> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for BorrowedWriteHalf<'a>, tokio_nofd_lt 'a TokioUdStreamWriteHalf<'a>);
derive_asraw!(unix: BorrowedWriteHalf<'_>);

/// Owned write half of a [`UdStream`](super::UdStream), created by [`.into_split()`](super::UdStream::into_split).
#[derive(Debug)]
pub struct OwnedWriteHalf(pub(super) TokioUdStreamOwnedWriteHalf);
impl OwnedWriteHalf {
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the two halves originated from the same call to [`.into_split()`](UdStream::into_split).
    pub fn reunite_with(self, read: OwnedReadHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(read, self)
    }

    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn get_peer_credentials(&self) -> io::Result<libc::ucred> {
        c_wrappers::get_peer_ucred(self.as_stream_fd())
    }

    /// Shuts down the write half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_fd(), Shutdown::Write)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the write half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_fd(&self) -> BorrowedFd<'_> {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamOwnedWriteHalf> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncWrite for OwnedWriteHalf {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
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
impl AsyncWrite for OwnedWriteHalf {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
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
impl AsFd for OwnedWriteHalf {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for OwnedWriteHalf, tokio_nofd TokioUdStreamOwnedWriteHalf);
derive_asraw!(unix: OwnedWriteHalf);
