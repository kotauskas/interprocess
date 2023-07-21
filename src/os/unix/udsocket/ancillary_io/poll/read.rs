use super::assert_future;
use crate::os::unix::udsocket::{cmsg::*, ReadAncillarySuccess};
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
fn _assert_async_read_ancillary_object_safe<'j: 'm + 'c, 'm, 'c, T: AsyncReadAncillary<DynCmsgMut<'m, 'c>> + 'j>(
    x: &mut T,
) -> &mut (dyn AsyncReadAncillary<DynCmsgMut<'m, 'c>> + 'j) {
    x as _
}

impl<AB: CmsgMut + ?Sized, P: DerefMut + Unpin> AsyncReadAncillary<AB> for Pin<P>
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

impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> AsyncReadAncillary<AB> for &mut T {
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
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> AsyncReadAncillary<AB> for Box<T> {
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
    /// Analogous to [`AsyncReadExt::read()`](futures_util::AsyncReadExt::read), but also reads control messages into
    /// the given ancillary buffer.
    ///
    /// The return value contains both the amount of main-band data read into the given regular buffer and the number of
    /// bytes read into the ancillary buffer.
    #[inline(always)]
    fn read_ancillary<'slf, 'b, 'ab>(
        &'slf mut self,
        buf: &'b mut [u8],
        abuf: &'ab mut AB,
    ) -> super::futures::ReadAncillary<'slf, 'b, 'ab, AB, Self>
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
    fn read_ancillary_vectored<'slf, 'b, 'iov, 'ab>(
        &'slf mut self,
        bufs: &'b mut [IoSliceMut<'iov>],
        abuf: &'ab mut AB,
    ) -> super::futures::ReadAncillaryVectored<'slf, 'b, 'iov, 'ab, AB, Self>
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
    fn read_ancillary_to_end<'slf, 'b, 'ab>(
        &'slf mut self,
        buf: &'b mut Vec<u8>,
        abuf: &'ab mut AB,
    ) -> super::futures::ReadToEndAncillary<'slf, 'b, 'ab, AB, Self>
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
    fn read_to_end_with_ancillary<'slf, 'b, 'ab>(
        &'slf mut self,
        buf: &'b mut Vec<u8>,
        abuf: &'ab mut AB,
    ) -> super::futures::ReadToEndAncillary<'slf, 'b, 'ab, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadToEndAncillary::new(self, buf, abuf, false))
    }

    /// Analogous to [`AsyncReadExt::read_exact()`](futures_util::AsyncReadExt::read_exact), but also reads ancillary
    /// data into the given buffer.
    fn read_exact_with_ancillary<'slf, 'b, 'ab>(
        &'slf mut self,
        buf: &'b mut [u8],
        abuf: &'ab mut AB,
    ) -> super::futures::ReadExactWithAncillary<'slf, 'b, 'ab, AB, Self>
    where
        Self: Unpin,
    {
        assert_future(super::futures::ReadExactWithAncillary::new(self, buf, abuf))
    }
}
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + ?Sized> AsyncReadAncillaryExt<AB> for T {}
