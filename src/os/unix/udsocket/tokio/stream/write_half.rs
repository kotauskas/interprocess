use super::{c_wrappers, poll_write_ancvec_ref, poll_write_ref, poll_write_vec_ref, ReadHalf, ReuniteError, UdStream};
use crate::os::unix::{
    udsocket::{cmsg::CmsgRef, poll::write_in_terms_of_vectored, AsyncWriteAncillary},
    unixprelude::*,
};
use futures_io::AsyncWrite;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{io::AsyncWrite as TokioAsyncWrite, net::unix::OwnedWriteHalf as TokioUdStreamWriteHalf};

/// Write half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct WriteHalf(pub(super) TokioUdStreamWriteHalf);
impl WriteHalf {
    /// Attempts to put two halves of a stream back together and recover the original stream. Succeeds only if the two
    /// halves originated from the same call to [`.split()`](UdStream::split).
    pub fn reunite_with(self, read: ReadHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(read, self)
    }

    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure
    /// contains the process identifier, user identifier and group identifier of the peer.
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
        c_wrappers::get_peer_ucred(self.as_fd())
    }

    /// Shuts down the write half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the
    /// second time it is called, depending on the platform. You must either avoid using the same value twice or ignore
    /// the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_fd(), Shutdown::Write)
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamWriteHalf> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncWrite for &WriteHalf {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        poll_write_ref(self.0.as_ref(), cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        poll_write_vec_ref(self.0.as_ref(), cx, bufs)
    }
    /// True.
    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        true
    }
}

impl AsyncWrite for &WriteHalf {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        <Self as TokioAsyncWrite>::poll_write(self, cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        <Self as TokioAsyncWrite>::poll_write_vectored(self, cx, bufs)
    }
}

impl AsyncWriteAncillary for &WriteHalf {
    #[inline]
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_>,
    ) -> Poll<io::Result<usize>> {
        write_in_terms_of_vectored(self, cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
        abuf: CmsgRef<'_>,
    ) -> Poll<io::Result<usize>> {
        poll_write_ancvec_ref(self.0.as_ref(), cx, bufs, abuf)
    }
}

impl TokioAsyncWrite for WriteHalf {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        self.pinproject().poll_write_vectored(cx, bufs)
    }
    /// True.
    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        true
    }
}

impl AsyncWrite for WriteHalf {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown()?;
        Poll::Ready(Ok(()))
    }
    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        <Self as TokioAsyncWrite>::poll_write_vectored(self, cx, bufs)
    }
}

impl AsyncWriteAncillary for WriteHalf {
    #[inline]
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_>,
    ) -> Poll<io::Result<usize>> {
        write_in_terms_of_vectored(self, cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
        abuf: CmsgRef<'_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}

impl AsFd for WriteHalf {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for WriteHalf, tokio_nofd TokioUdStreamWriteHalf);
derive_asraw!(unix: WriteHalf);
