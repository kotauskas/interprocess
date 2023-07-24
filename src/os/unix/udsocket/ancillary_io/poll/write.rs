use crate::os::unix::udsocket::{cmsg::*, WithCmsgMut, WithCmsgRef};
use futures_core::ready;
use futures_io::*;
use std::{
    io::{self, IoSlice},
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

/// An extension of [`AsyncWrite`] that enables operations involving ancillary data.
pub trait AsyncWriteAncillary: AsyncWrite {
    /// Analogous to [`AsyncWrite::poll_write()`], but also sends control messages from the given ancillary buffer.
    ///
    /// The return value only the amount of main-band data sent from the given regular buffer – the entirety of the
    /// given `abuf` is always sent in full.
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>>;

    /// Same as [`poll_write_ancillary`](AsyncWriteAncillary::poll_write_ancillary), but performs a
    /// [gather write](https://en.wikipedia.org/wiki/Vectored_I%2FO) instead.
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        let buf = bufs.iter().find(|b| !b.is_empty()).map_or(&[][..], |b| &**b);
        self.poll_write_ancillary(cx, buf, abuf)
    }
}

pub(crate) fn write_in_terms_of_vectored(
    slf: Pin<&mut impl AsyncWriteAncillary>,
    cx: &mut Context<'_>,
    buf: &[u8],
    abuf: CmsgRef<'_, '_>,
) -> Poll<io::Result<usize>> {
    slf.poll_write_ancillary_vectored(cx, &[IoSlice::new(buf)], abuf)
}

#[cfg(debug_assertions)]
fn _assert_ext<AWA: AsyncWriteAncillaryExt + ?Sized>(x: &mut AWA) -> &mut AWA {
    x
}
#[cfg(debug_assertions)]
fn _assert_async_write_ancillary_object_safe<'a, AWA: AsyncWriteAncillary + 'a>(
    x: &mut AWA,
) -> &mut (dyn AsyncWriteAncillary + 'a) {
    _assert_ext(x as _)
}

impl<P: DerefMut + Unpin> AsyncWriteAncillary for Pin<P>
where
    P::Target: AsyncWriteAncillary,
{
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        self.get_mut().as_mut().poll_write_ancillary(cx, buf, abuf)
    }
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        self.get_mut().as_mut().poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}

impl<AWA: AsyncWriteAncillary + Unpin + ?Sized> AsyncWriteAncillary for &mut AWA {
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self.get_mut()).poll_write_ancillary(cx, buf, abuf)
    }
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self.get_mut()).poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}
impl<AWA: AsyncWriteAncillary + Unpin + ?Sized> AsyncWriteAncillary for Box<AWA> {
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self.get_mut()).poll_write_ancillary(cx, buf, abuf)
    }
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self.get_mut()).poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}

/// Methods derived from the interface of [`AsyncWriteAncillary`].
pub trait AsyncWriteAncillaryExt: AsyncWriteAncillary {
    /// The asynchronous version of
    /// [`WriteAncillaryExt::with_cmsg_ref`](super::super::WriteAncillaryExt::with_cmsg_ref).
    #[inline(always)]
    fn with_cmsg_ref<'writer, 'abuf, 'acol>(
        &'writer mut self,
        abuf: CmsgRef<'abuf, 'acol>,
    ) -> WithCmsgRef<'abuf, 'acol, &'writer mut Self>
    where
        Self: Unpin,
    {
        AsyncWriteAncillaryExt::with_cmsg_ref_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_ref()`](AsyncWriteAncillaryExt::with_cmsg_ref), but does not require that `Self: Unpin`,
    /// instead requiring the caller to pass `self` by `Pin`.
    #[inline(always)]
    fn with_cmsg_ref_pin<'writer, 'abuf, 'acol>(
        self: Pin<&'writer mut Self>,
        abuf: CmsgRef<'abuf, 'acol>,
    ) -> WithCmsgRef<'abuf, 'acol, Pin<&'writer mut Self>> {
        AsyncWriteAncillaryExt::with_cmsg_ref_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_ref()`](AsyncWriteAncillaryExt::with_cmsg_ref), but does not borrow `self`, consuming
    /// ownership instead.
    #[inline(always)]
    fn with_cmsg_ref_by_val<'abuf, 'acol>(self, abuf: CmsgRef<'abuf, 'acol>) -> WithCmsgRef<'abuf, 'acol, Self>
    where
        Self: Unpin + Sized,
    {
        WithCmsgRef { writer: self, abuf }
    }

    /// Analogous to [`AsyncWrite::poll_write()`], but also sends control messages from the given ancillary buffer.
    ///
    /// The return value only the amount of main-band data sent from the given regular buffer – the entirety of the
    /// given `abuf` is always sent in full.
    fn write_ancillary<'slf, 'b, 'ab, 'ac>(
        &'slf mut self,
        buf: &'b [u8],
        abuf: CmsgRef<'ab, 'ac>,
    ) -> super::futures::WriteAncillary<'slf, 'b, 'ab, 'ac, Self>
    where
        Self: Unpin,
    {
        super::futures::WriteAncillary::new(self, buf, abuf)
    }
    /// Same as [`write_ancillary`](AsyncWriteAncillaryExt::write_ancillary), but performs a
    /// [gather write](https://en.wikipedia.org/wiki/Vectored_I%2FO) instead.
    fn write_ancillary_vectored<'slf, 'bufs, 'iov, 'ab, 'ac>(
        &'slf mut self,
        bufs: &'bufs [IoSlice<'iov>],
        abuf: CmsgRef<'ab, 'ac>,
    ) -> super::futures::WriteAncillaryVectored<'slf, 'bufs, 'iov, 'ab, 'ac, Self>
    where
        Self: Unpin,
    {
        super::futures::WriteAncillaryVectored::new(self, bufs, abuf)
    }
    /// Analogous to [`write_all`](futures_util::AsyncWriteExt::write_all), but also writes ancillary data.
    fn write_all_ancillary<'slf, 'b, 'ab, 'ac>(
        &'slf mut self,
        buf: &'b [u8],
        abuf: CmsgRef<'ab, 'ac>,
    ) -> super::futures::WriteAllAncillary<'slf, 'b, 'ab, 'ac, Self>
    where
        Self: Unpin,
    {
        super::futures::WriteAllAncillary::new(self, buf, abuf)
    }
}
// hi myrl
impl<AWA: AsyncWriteAncillary + ?Sized> AsyncWriteAncillaryExt for AWA {}

impl<AWA: AsyncWriteAncillary + Unpin> AsyncWrite for WithCmsgRef<'_, '_, AWA> {
    /// Writes via [`.poll_write_ancillary()`](AsyncWriteAncillary::poll_write_ancillary) of the inner writer with the
    /// `abuf` argument being `self.abuf`; if `abuf` is empty, [`.poll_write()`](AsyncWrite::poll_write) of the inner
    /// writer is simply used.
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let slf = self.get_mut();
        let writer = Pin::new(&mut slf.writer);

        let bytes_written = if !slf.abuf.inner().is_empty() {
            let bw = ready!(writer.poll_write_ancillary(cx, buf, slf.abuf))?;
            slf.abuf.consume_bytes(slf.abuf.inner().len());
            bw
        } else {
            ready!(writer.poll_write(cx, buf))?
        };
        Poll::Ready(Ok(bytes_written))
    }

    /// Flushes the inner writer, which normally does nothing, since sockets can't be flushed.
    #[inline(always)]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.writer).poll_close(cx)
    }

    /// Writes via [`.poll_write_ancillary_vectored()`](AsyncWriteAncillary::poll_write_ancillary_vectored) of the inner
    /// writer with the `abuf` argument being `self.abuf`; if `abuf` is empty,
    /// [`.poll_write_vectored()`](AsyncWrite::poll_write_vectored) of the inner writer is simply used.
    fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[IoSlice<'_>]) -> Poll<Result<usize>> {
        let slf = self.get_mut();
        let writer = Pin::new(&mut slf.writer);

        let bytes_written = if !slf.abuf.inner().is_empty() {
            let bw = ready!(writer.poll_write_ancillary_vectored(cx, bufs, slf.abuf))?;
            slf.abuf.consume_bytes(slf.abuf.inner().len());
            bw
        } else {
            ready!(writer.poll_write_vectored(cx, bufs))?
        };
        Poll::Ready(Ok(bytes_written))
    }
}

/// Forwarding of `AsyncWrite` through an irrelevant adapter.
impl<AW: AsyncWrite + Unpin, AB: ?Sized> AsyncWrite for WithCmsgMut<'_, AW, AB> {
    #[inline(always)]
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Pin::new(&mut self.reader).poll_write(cx, buf)
    }
    #[inline(always)]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.reader).poll_flush(cx)
    }
    #[inline(always)]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.reader).poll_close(cx)
    }
    #[inline(always)]
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.reader).poll_write_vectored(cx, bufs)
    }
}

/// Forwarding of `AsyncWriteAncillary` through an irrelevant adapter.
impl<AWA: AsyncWriteAncillary + Unpin, AB: ?Sized> AsyncWriteAncillary for WithCmsgMut<'_, AWA, AB> {
    #[inline(always)]
    fn poll_write_ancillary(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.reader).poll_write_ancillary(cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_write_ancillary_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.reader).poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}
