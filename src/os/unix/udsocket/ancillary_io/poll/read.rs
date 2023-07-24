use super::assert_future;
use crate::os::unix::udsocket::{cmsg::*, ReadAncillarySuccess, WithCmsgMut};
use futures_core::ready;
use futures_io::*;
use std::{
    io::{self, IoSliceMut},
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

/// An extension of [`AsyncRead`] that enables operations involving ancillary data – the async equivalent of
/// [`ReadAncillary`](super::ReadAncillary).
///
/// The generic parameter on the trait allows for trait objects to be constructed. Simply substitute [`DynCmsgMut`] or
/// [`DynCmsgMutStatic`] for `AB` to obtain an object-safe `AsyncReadAncillary`.
pub trait AsyncReadAncillary<AB: CmsgMut + ?Sized>: AsyncRead {
    /// Analogous to [`AsyncRead::poll_read()`], but also reads control messages into the given ancillary buffer.
    ///
    /// The return value contains both the amount of main-band data read into the given regular buffer and the number of
    /// bytes read into the ancillary buffer.
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>>;

    /// Same as [`read_ancillary`](AsyncReadAncillary::poll_read_ancillary), but performs a
    /// [scatter read](https://en.wikipedia.org/wiki/Vectored_I%2FO) instead.
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        let buf = bufs
            .iter_mut()
            .find(|b| !b.is_empty())
            .map_or(&mut [][..], |b| &mut **b);
        self.poll_read_ancillary(cx, buf, abuf)
    }
}

pub(crate) fn read_in_terms_of_vectored<AB: CmsgMut + ?Sized>(
    slf: Pin<&mut impl AsyncReadAncillary<AB>>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
    abuf: &mut AB,
) -> Poll<io::Result<ReadAncillarySuccess>> {
    slf.poll_read_ancillary_vectored(cx, &mut [IoSliceMut::new(buf)], abuf)
}

#[cfg(debug_assertions)]
fn _assert_ext<ARA: AsyncReadAncillaryExt<AB> + ?Sized, AB: CmsgMut + ?Sized>(x: &mut ARA) -> &mut ARA {
    x
}
#[cfg(debug_assertions)]
fn _assert_async_read_ancillary_object_safe<'j: 'm + 'c, 'm, 'c, ARA: AsyncReadAncillary<DynCmsgMut<'m, 'c>> + 'j>(
    x: &mut ARA,
) -> &mut (dyn AsyncReadAncillary<DynCmsgMut<'m, 'c>> + 'j) {
    _assert_ext(x as _)
}

impl<P: DerefMut + Unpin, AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for Pin<P>
where
    P::Target: AsyncReadAncillary<AB>,
{
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        self.get_mut().as_mut().poll_read_ancillary(cx, buf, abuf)
    }
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        self.get_mut().as_mut().poll_read_ancillary_vectored(cx, bufs, abuf)
    }
}

impl<ARA: AsyncReadAncillary<AB> + Unpin + ?Sized, AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for &mut ARA {
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut **self.get_mut()).poll_read_ancillary(cx, buf, abuf)
    }
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut **self.get_mut()).poll_read_ancillary_vectored(cx, bufs, abuf)
    }
}
impl<ARA: AsyncReadAncillary<AB> + Unpin + ?Sized, AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for Box<ARA> {
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut **self.get_mut()).poll_read_ancillary(cx, buf, abuf)
    }
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut **self.get_mut()).poll_read_ancillary_vectored(cx, bufs, abuf)
    }
}

/// Methods derived from the interface of [`AsyncReadAncillary`].
///
/// See the documentation on `AsyncReadAncillary` for notes on why a type parameter is present.
pub trait AsyncReadAncillaryExt<AB: CmsgMut + ?Sized>: AsyncReadAncillary<AB> {
    /// The asynchronous version of [`ReadAncillaryExt::with_cmsg_mut`](super::super::ReadAncillaryExt::with_cmsg_mut).
    #[inline(always)]
    fn with_cmsg_mut<'reader, 'abuf>(
        &'reader mut self,
        abuf: &'abuf mut AB,
    ) -> WithCmsgMut<'abuf, &'reader mut Self, AB>
    where
        Self: Unpin,
    {
        AsyncReadAncillaryExt::with_cmsg_mut_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_mut()`](AsyncReadAncillaryExt::with_cmsg_mut), but does not require that `Self: Unpin`,
    /// instead requiring the caller to pass `self` by `Pin`.
    #[inline(always)]
    fn with_cmsg_mut_pin<'reader, 'abuf>(
        self: Pin<&'reader mut Self>,
        abuf: &'abuf mut AB,
    ) -> WithCmsgMut<'abuf, Pin<&'reader mut Self>, AB> {
        AsyncReadAncillaryExt::with_cmsg_mut_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_mut()`](AsyncReadAncillaryExt::with_cmsg_mut), but does not borrow `self`, consuming ownership
    /// instead.
    #[inline(always)]
    fn with_cmsg_mut_by_val(self, abuf: &mut AB) -> WithCmsgMut<'_, Self, AB>
    where
        Self: Unpin + Sized,
    {
        WithCmsgMut::new(self, abuf)
    }

    /// Analogous to [`AsyncReadExt::read()`](futures_util::AsyncReadExt::read), but also reads control messages into
    /// the given ancillary buffer.
    ///
    /// The return value contains both the amount of main-band data read into the given regular buffer and the number of
    /// bytes read into the ancillary buffer.
    #[inline(always)]
    fn read_ancillary<'reader, 'buf, 'abuf>(
        &'reader mut self,
        buf: &'buf mut [u8],
        abuf: &'abuf mut AB,
    ) -> super::futures::ReadAncillary<'reader, 'buf, 'abuf, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadAncillary::new(self, buf, abuf))
    }
    /// Analogous to [`AsyncReadExt::read_vectored()`](futures_util::AsyncReadExt::read_vectored), but also reads
    /// control messages into the given ancillary buffer.
    ///
    /// The return value contains both the amount of main-band data read into the given regular buffers and the number
    /// of bytes read into the ancillary buffer.
    fn read_ancillary_vectored<'reader, 'bufs, 'iovec, 'abuf>(
        &'reader mut self,
        bufs: &'bufs mut [IoSliceMut<'iovec>],
        abuf: &'abuf mut AB,
    ) -> super::futures::ReadAncillaryVectored<'reader, 'bufs, 'iovec, 'abuf, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadAncillaryVectored::new(self, bufs, abuf))
    }
    /// Analogous to [`AsyncReadExt::read_to_end()`](futures_util::AsyncReadExt::read_to_end), but also reads ancillary
    /// data into the given ancillary buffer, growing it with the regular data buffer.
    ///
    /// **Read-to-end semantics apply to both main and ancillary data**, unlike with [`read_to_end_with_ancillary()`],
    /// which only grows the main data buffer and reads ancillary data exactly the same way as a regular
    /// [`read_ancillary`](AsyncReadAncillaryExt::read_ancillary) operation would.
    ///
    /// Note that using a buffer type that doesn't support resizing, such as [`CmsgMutBuf`], will produce identical
    /// behavior to [`read_to_end_with_ancillary()`].
    ///
    /// [`read_to_end_with_ancillary()`]: AsyncReadAncillaryExt::read_to_end_with_ancillary
    #[inline(always)]
    fn read_ancillary_to_end<'reader, 'buf, 'abuf>(
        &'reader mut self,
        buf: &'buf mut Vec<u8>,
        abuf: &'abuf mut AB,
    ) -> super::futures::ReadToEndAncillary<'reader, 'buf, 'abuf, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadToEndAncillary::new(self, buf, abuf, true))
    }
    /// Analogous to [`AsyncReadExt::read_to_end()`](futures_util::AsyncReadExt::read_to_end), but also reads ancillary
    /// data into the given ancillary buffer.
    ///
    /// **Read-to-end semantics apply only to the main data**, unlike with
    /// [`read_ancillary_to_end()`](AsyncReadAncillaryExt::read_ancillary_to_end), which grows both buffers adaptively
    /// and thus requires both of them to be passed with ownership.
    #[inline(always)]
    fn read_to_end_with_ancillary<'reader, 'buf, 'abuf>(
        &'reader mut self,
        buf: &'buf mut Vec<u8>,
        abuf: &'abuf mut AB,
    ) -> super::futures::ReadToEndAncillary<'reader, 'buf, 'abuf, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadToEndAncillary::new(self, buf, abuf, false))
    }

    /// Analogous to [`AsyncReadExt::read_exact()`](futures_util::AsyncReadExt::read_exact), but also reads ancillary
    /// data into the given buffer.
    fn read_exact_with_ancillary<'reader, 'buf, 'abuf>(
        &'reader mut self,
        buf: &'buf mut [u8],
        abuf: &'abuf mut AB,
    ) -> super::futures::ReadExactWithAncillary<'reader, 'buf, 'abuf, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadExactWithAncillary::new(self, buf, abuf))
    }
}
impl<ARA: AsyncReadAncillary<AB> + ?Sized, AB: CmsgMut + ?Sized> AsyncReadAncillaryExt<AB> for ARA {}

impl<ARA: AsyncReadAncillary<AB> + Unpin, AB: CmsgMut + ?Sized> AsyncRead for WithCmsgMut<'_, ARA, AB> {
    /// Reads via [`.poll_read_ancillary()`](AsyncReadAncillary::poll_read_ancillary) on the inner reader with the
    /// `abuf` argument being `self.abuf`.
    ///
    /// If `reserve` is enabled, it will be resized to match or exceed the size of `buf` (if
    /// possible) via [`.reserve_up_to_exact()`](CmsgMutExt::reserve_up_to_exact).
    ///
    /// Only the amount of data read into `buf` is returned, with the amount of ancillary data read being stored in the
    /// adapter to be later retrieved via [`.total_read()`](WithCmsgMut::total_read).
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let slf = self.get_mut();
        slf.maybe_reserve(buf.len());
        let sc = ready!(Pin::new(&mut slf.reader).poll_read_ancillary(cx, buf, slf.abuf))?;
        slf.accumulator += sc;
        Poll::Ready(Ok(sc.main))
    }

    /// Reads via [`.poll_read_ancillary_vectored()`](AsyncReadAncillary::poll_read_ancillary_vectored) on the inner
    /// reader with the `abuf` argument being `self.abuf`.
    ///
    /// If `reserve` is enabled, it will be resized to match or exceed the size of the ***last*** buffer in `bufs` (if
    /// possible) via [`.reserve_up_to_exact()`](CmsgMutExt::reserve_up_to_exact).
    ///
    /// Only the amount of data read into `bufs` is returned, with the amount of ancillary data read being stored in the
    /// adapter to be later retrieved via [`.total_read()`](WithCmsgMut::total_read).
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        let slf = self.get_mut();
        if let Some(s) = bufs.last() {
            slf.maybe_reserve(s.len());
        }
        let sc = ready!(Pin::new(&mut slf.reader).poll_read_ancillary_vectored(cx, bufs, slf.abuf))?;
        slf.accumulator += sc;
        Poll::Ready(Ok(sc.main))
    }
}
