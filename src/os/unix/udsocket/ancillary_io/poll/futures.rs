//! [Futures](Future) returned by [`AsyncReadAncillaryExt`] and [`AsyncWriteAncillaryExt`].

use super::{
    super::{AsyncReadAncillary, AsyncWriteAncillary, ReadAncillarySuccess},
    AsyncReadAncillaryExt, AsyncWriteAncillaryExt,
};
use crate::os::unix::udsocket::{
    cmsg::{CmsgMut, CmsgRef},
    WithCmsgMut, WithCmsgRef,
};
use futures_core::ready;
use futures_io::{AsyncRead, AsyncWrite};
use futures_util::io::AsyncReadExt;
use std::{
    future::Future,
    io::{self, IoSlice, IoSliceMut},
    marker::PhantomData,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

/// [Future] returned by [`read_ancillary()`](super::AsyncReadAncillaryExt::read_ancillary).
pub struct ReadAncillary<'reader, 'buf, 'abuf, AB: ?Sized, ARA: ?Sized> {
    slf: &'reader mut ARA,
    buf: &'buf mut [u8],
    abuf: &'abuf mut AB,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'reader, 'buf, 'abuf, AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadAncillary<'reader, 'buf, 'abuf, AB, ARA>
{
    #[inline(always)]
    pub(super) fn new(slf: &'reader mut ARA, buf: &'buf mut [u8], abuf: &'abuf mut AB) -> Self {
        Self {
            slf,
            buf,
            abuf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized> Future for ReadAncillary<'_, '_, '_, AB, ARA> {
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_read_ancillary(cx, slf.buf, slf.abuf)
    }
}

/// [Future] returned by [`read_ancillary_vectored()`](super::AsyncReadAncillaryExt::read_ancillary_vectored).
pub struct ReadAncillaryVectored<'reader, 'buf, 'iov, 'abuf, AB: ?Sized, ARA: ?Sized> {
    slf: &'reader mut ARA,
    bufs: &'buf mut [IoSliceMut<'iov>],
    abuf: &'abuf mut AB,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'reader, 'buf, 'iov, 'abuf, AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadAncillaryVectored<'reader, 'buf, 'iov, 'abuf, AB, ARA>
{
    #[inline(always)]
    pub(super) fn new(slf: &'reader mut ARA, bufs: &'buf mut [IoSliceMut<'iov>], abuf: &'abuf mut AB) -> Self {
        Self {
            slf,
            bufs,
            abuf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadAncillaryVectored<'_, '_, '_, '_, AB, ARA>
{
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_read_ancillary_vectored(cx, slf.bufs, slf.abuf)
    }
}

//--- Actual adapters ---

/// [Future] returned by [`read_ancillary_to_end()`](super::AsyncReadAncillaryExt::read_ancillary_to_end) and
/// [`read_to_end_with_ancillary()`](super::AsyncReadAncillaryExt::read_to_end_with_ancillary).
pub struct ReadToEndAncillary<'reader, 'buf, 'abuf, AB: ?Sized, ARA: ?Sized> {
    partappl: WithCmsgMut<'abuf, &'reader mut ARA, AB>,
    buf: &'buf mut Vec<u8>,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'reader, 'buf, 'abuf, AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadToEndAncillary<'reader, 'buf, 'abuf, AB, ARA>
{
    #[inline(always)]
    pub(super) fn new(reader: &'reader mut ARA, buf: &'buf mut Vec<u8>, abuf: &'abuf mut AB, reserve: bool) -> Self {
        let mut ret = Self {
            partappl: reader.with_cmsg_mut(abuf),
            buf,
            _phantom: PhantomData,
        };
        ret.partappl.reserve = reserve;
        ret
    }
}
impl<AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadToEndAncillary<'_, '_, '_, AB, ARA>
{
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        // Ephemeral future is fine here because it doesn't keep any state that we care about; in
        // fact, a close inspection of the source code reveals that the only piece of state being
        // kept between polls is `start_len` used to calculate the final return value. We
        // circumvent that entire feature altogether by using the `partappl`'s own internal
        // counter.
        let mut rte = slf.partappl.read_to_end(slf.buf);
        ready!(Pin::new(&mut rte).poll(cx))?;
        Poll::Ready(Ok(slf.partappl.total_read()))
    }
}

/// [Future] returned by [`read_exact_with_ancillary()`](super::AsyncReadAncillaryExt::read_exact_with_ancillary).
pub struct ReadExactWithAncillary<'reader, 'buf, 'abuf, AB: ?Sized, ARA: ?Sized> {
    partappl: WithCmsgMut<'abuf, &'reader mut ARA, AB>,
    buf: &'buf mut [u8],
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'reader, 'buf, 'abuf, AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadExactWithAncillary<'reader, 'buf, 'abuf, AB, ARA>
{
    #[inline(always)]
    pub(super) fn new(reader: &'reader mut ARA, buf: &'buf mut [u8], abuf: &'abuf mut AB) -> Self {
        let mut ret = Self {
            partappl: reader.with_cmsg_mut(abuf),
            buf,
            _phantom: PhantomData,
        };
        ret.partappl.reserve = false;
        ret
    }
}
impl<AB: CmsgMut + ?Sized, ARA: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadExactWithAncillary<'_, '_, '_, AB, ARA>
{
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();

        // The below code was transcluded from futures_util, file src/io/read_exact.rs.
        while !slf.buf.is_empty() {
            let n = ready!(Pin::new(&mut slf.partappl).poll_read(cx, slf.buf))?;
            {
                let (_, rest) = mem::take(&mut slf.buf).split_at_mut(n);
                slf.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()));
            }
        }
        Poll::Ready(Ok(slf.partappl.total_read()))
    }
}

/// [Future] returned by [`write_ancillary()`](super::AsyncWriteAncillaryExt::write_ancillary).
pub struct WriteAncillary<'writer, 'buf, 'abuf, AWA: ?Sized> {
    slf: &'writer mut AWA,
    buf: &'buf [u8],
    abuf: CmsgRef<'abuf>,
}
impl<'writer, 'buf, 'abuf, AWA: AsyncWriteAncillary + Unpin + ?Sized> WriteAncillary<'writer, 'buf, 'abuf, AWA> {
    #[inline(always)]
    pub(super) fn new(slf: &'writer mut AWA, buf: &'buf [u8], abuf: CmsgRef<'abuf>) -> Self {
        Self { slf, buf, abuf }
    }
}
impl<AWA: AsyncWriteAncillary + Unpin + ?Sized> Future for WriteAncillary<'_, '_, '_, AWA> {
    type Output = io::Result<usize>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_write_ancillary(cx, slf.buf, slf.abuf)
    }
}

/// [Future] returned by [`write_ancillary_vectored()`](super::AsyncWriteAncillaryExt::write_ancillary_vectored).
pub struct WriteAncillaryVectored<'writer, 'buf, 'iov, 'abuf, AWA: ?Sized> {
    slf: &'writer mut AWA,
    bufs: &'buf [IoSlice<'iov>],
    abuf: CmsgRef<'abuf>,
}
impl<'writer, 'buf, 'iov, 'abuf, AWA: AsyncWriteAncillary + Unpin + ?Sized>
    WriteAncillaryVectored<'writer, 'buf, 'iov, 'abuf, AWA>
{
    #[inline(always)]
    pub(super) fn new(slf: &'writer mut AWA, bufs: &'buf [IoSlice<'iov>], abuf: CmsgRef<'abuf>) -> Self {
        Self { slf, bufs, abuf }
    }
}
impl<AWA: AsyncWriteAncillary + Unpin + ?Sized> Future for WriteAncillaryVectored<'_, '_, '_, '_, AWA> {
    type Output = io::Result<usize>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_write_ancillary_vectored(cx, slf.bufs, slf.abuf)
    }
}

//--- Actual adapters ---

/// [Future] returned by [`write_all_ancillary()`](super::AsyncWriteAncillaryExt::write_all_ancillary).
pub struct WriteAllAncillary<'writer, 'buf, 'abuf, AWA: ?Sized> {
    partappl: WithCmsgRef<'abuf, &'writer mut AWA>,
    buf: &'buf [u8],
}
impl<'writer, 'buf, 'abuf, AWA: AsyncWriteAncillary + Unpin + ?Sized> WriteAllAncillary<'writer, 'buf, 'abuf, AWA> {
    #[inline(always)]
    pub(super) fn new(writer: &'writer mut AWA, buf: &'buf [u8], abuf: CmsgRef<'abuf>) -> Self {
        Self {
            partappl: writer.with_cmsg_ref(abuf),
            buf,
        }
    }
}
impl<AWA: AsyncWriteAncillary + Unpin + ?Sized> Future for WriteAllAncillary<'_, '_, '_, AWA> {
    type Output = io::Result<()>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();

        // The below code was transcluded from futures_util, file src/io/write_all.rs.
        while !slf.buf.is_empty() {
            let n = ready!(Pin::new(&mut slf.partappl).poll_write(cx, slf.buf))?;
            {
                let (_, rest) = mem::take(&mut slf.buf).split_at(n);
                slf.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }

        Poll::Ready(Ok(()))
    }
}
