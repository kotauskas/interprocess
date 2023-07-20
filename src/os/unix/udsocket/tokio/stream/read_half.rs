use super::{c_wrappers, poll_read_ancvec_ref, poll_read_ref, poll_read_vec_ref, ReuniteError, UdStream, WriteHalf};
use crate::os::unix::{
    udsocket::{cmsg::CmsgMut, poll::read_in_terms_of_vectored, AsyncReadAncillary, ReadAncillarySuccess},
    unixprelude::*,
};
use futures_io::AsyncRead;
use std::{
    io,
    net::Shutdown,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead as TokioAsyncRead, ReadBuf as TokioReadBuf},
    net::unix::OwnedReadHalf as TokioUdStreamReadHalf,
};

/// Read half of a [`UdStream`](super::UdStream), created by [`.split()`](super::UdStream::split).
#[derive(Debug)]
pub struct ReadHalf(pub(super) TokioUdStreamReadHalf);
impl ReadHalf {
    /// Attempts to put two halves of a stream back together and recover the original stream. Succeeds only if the two
    /// halves originated from the same call to [`.split()`](UdStream::split).
    pub fn reunite_with(self, write: WriteHalf) -> Result<UdStream, ReuniteError> {
        UdStream::reunite(self, write)
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

    /// Shuts down the read half.
    ///
    /// Attempting to call this method multiple times may return `Ok(())` every time or it may return an error the
    /// second time it is called, depending on the platform. You must either avoid using the same value twice or ignore
    /// the error entirely.
    pub fn shutdown(&self) -> io::Result<()> {
        c_wrappers::shutdown(self.as_fd(), Shutdown::Read)
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStreamReadHalf> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl TokioAsyncRead for &ReadHalf {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        poll_read_ref(self.0.as_ref(), cx, buf)
    }
}

impl AsyncRead for &ReadHalf {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        <Self as TokioAsyncRead>::poll_read(self, cx, &mut buf).map(|p| p.map(|()| buf.filled().len()))
    }

    #[inline(always)]
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        poll_read_vec_ref(self.0.as_ref(), cx, bufs)
    }
}

impl<AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for &ReadHalf {
    #[inline]
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        read_in_terms_of_vectored(self, cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        poll_read_ancvec_ref(self.0.as_ref(), cx, bufs, abuf)
    }
}

// TODO the rest of by-ref, same for write half

impl TokioAsyncRead for ReadHalf {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}

impl AsyncRead for ReadHalf {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        self.pinproject()
            .poll_read(cx, &mut buf)
            .map(|p| p.map(|()| buf.filled().len()))
    }
    #[inline(always)]
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        <&Self as AsyncRead>::poll_read_vectored(Pin::new(&mut &*self), cx, bufs)
    }
}

impl<AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for ReadHalf {
    #[inline(always)]
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut &*self).poll_read_ancillary(cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut &*self).poll_read_ancillary_vectored(cx, bufs, abuf)
    }
}

impl AsFd for ReadHalf {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_ref().as_fd()
    }
}

tokio_wrapper_trait_impls!(
    for ReadHalf, tokio_nofd TokioUdStreamReadHalf);
derive_asraw!(unix: ReadHalf);
