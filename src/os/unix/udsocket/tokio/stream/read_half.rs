use super::{c_wrappers, OwnedWriteHalf, ReuniteError, UdStream};
use crate::os::unix::{
    udsocket::{ancwrap, cmsg::CmsgMut, poll::read_in_terms_of_vectored, AsyncReadAncillary},
    unixprelude::*,
};
use futures_core::ready;
use futures_io::AsyncRead;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead as TokioAsyncRead, ReadBuf as TokioReadBuf},
    net::{
        unix::{OwnedReadHalf as TokioUdStreamOwnedReadHalf, ReadHalf as TokioUdStreamReadHalf},
        UnixStream as TokioUdStream,
    },
};

// TODO remove borrowed halves

/// Borrowed read half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct BorrowedReadHalf<'a>(pub(super) TokioUdStreamReadHalf<'a>);

impl<'a> BorrowedReadHalf<'a> {
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
    /// Shuts down the read half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_fd(), Shutdown::Read)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the read half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_fd(&self) -> BorrowedFd<'_> {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamReadHalf<'a>> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncRead for BorrowedReadHalf<'_> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
impl AsyncRead for BorrowedReadHalf<'_> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl AsFd for BorrowedReadHalf<'_> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for BorrowedReadHalf<'a>, tokio_nofd_lt 'a TokioUdStreamReadHalf<'a>);
derive_asraw!(unix: BorrowedReadHalf<'_>);

/// Owned read half of a [`UdStream`](super::UdStream), created by [`.into_split()`](super::UdStream::into_split).
#[derive(Debug)]
pub struct OwnedReadHalf(pub(super) TokioUdStreamOwnedReadHalf);
impl OwnedReadHalf {
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the two halves originated from the same call to [`.into_split()`](UdStream::into_split).
    pub fn reunite_with(self, write: OwnedWriteHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(self, write)
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

    /// Shuts down the read half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_stream_fd(), Shutdown::Read)
    }

    /// Returns the underlying file descriptor. Note that this isn't a file descriptor for the read half specifically, but rather for the whole stream, so this isn't exposed as a struct method.
    fn as_stream_fd(&self) -> BorrowedFd<'_> {
        let stream: &TokioUdStream = self.0.as_ref();
        stream.as_fd()
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamOwnedReadHalf> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncRead for OwnedReadHalf {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}
impl AsyncRead for OwnedReadHalf {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        match self.pinproject().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for OwnedReadHalf {
    #[inline]
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<crate::os::unix::udsocket::ReadAncillarySuccess>> {
        read_in_terms_of_vectored(self, cx, buf, abuf)
    }
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<crate::os::unix::udsocket::ReadAncillarySuccess>> {
        let slf = self.get_mut();
        loop {
            match ancwrap::recvmsg(slf.as_fd(), bufs, abuf, None) {
                Ok(r) => return Poll::Ready(Ok(r)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            ready!(slf.0.as_ref().poll_read_ready(cx))?;
        }
    }
}

impl AsFd for OwnedReadHalf {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for OwnedReadHalf, tokio_nofd TokioUdStreamOwnedReadHalf);
derive_asraw!(unix: OwnedReadHalf);
