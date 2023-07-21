use crate::os::unix::udsocket::cmsg::*;
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
fn _assert_async_write_ancillary_object_safe<'a, T: AsyncWriteAncillary + 'a>(
    x: &mut T,
) -> &mut (dyn AsyncWriteAncillary + 'a) {
    x as _
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

impl<T: AsyncWriteAncillary + Unpin + ?Sized> AsyncWriteAncillary for &mut T {
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
impl<T: AsyncWriteAncillary + Unpin + ?Sized> AsyncWriteAncillary for Box<T> {
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
impl<T: AsyncWriteAncillary + ?Sized> AsyncWriteAncillaryExt for T {}
