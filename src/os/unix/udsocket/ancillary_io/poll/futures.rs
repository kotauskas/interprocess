//! [Futures](Future) returned by [`AsyncReadAncillaryExt`] and [`AsyncWriteAncillaryExt`].

use super::super::{AsyncReadAncillary, AsyncWriteAncillary, ReadAncillarySuccess};
use crate::os::unix::udsocket::cmsg::{CmsgMut, CmsgMutExt, CmsgRef};
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
pub struct ReadAncillary<'slf, 'b, 'ab, AB: ?Sized, T: ?Sized> {
    slf: &'slf mut T,
    buf: &'b mut [u8],
    abuf: &'ab mut AB,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'slf, 'b, 'ab, AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadAncillary<'slf, 'b, 'ab, AB, T>
{
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, buf: &'b mut [u8], abuf: &'ab mut AB) -> Self {
        Self {
            slf,
            buf,
            abuf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> Future for ReadAncillary<'_, '_, '_, AB, T> {
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_read_ancillary(cx, slf.buf, slf.abuf)
    }
}

/// [Future] returned by [`read_ancillary_vectored()`](super::AsyncReadAncillaryExt::read_ancillary_vectored).
pub struct ReadAncillaryVectored<'slf, 'b, 'iov, 'ab, AB: ?Sized, T: ?Sized> {
    slf: &'slf mut T,
    bufs: &'b mut [IoSliceMut<'iov>],
    abuf: &'ab mut AB,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'slf, 'b, 'iov, 'ab, AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadAncillaryVectored<'slf, 'b, 'iov, 'ab, AB, T>
{
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, bufs: &'b mut [IoSliceMut<'iov>], abuf: &'ab mut AB) -> Self {
        Self {
            slf,
            bufs,
            abuf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadAncillaryVectored<'_, '_, '_, '_, AB, T>
{
    type Output = io::Result<ReadAncillarySuccess>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_read_ancillary_vectored(cx, slf.bufs, slf.abuf)
    }
}

//--- Actual adapters ---

// Same business as the sync version.
struct ReadAncillaryPartAppl<'slf, 'ab, ARA: ?Sized, AB: ?Sized> {
    slf: Pin<&'slf mut ARA>,
    abuf: &'ab mut AB,
    /// An accumulator for the return value.
    ret: ReadAncillarySuccess,
    /// Whether to reserve together with the main-band buffer.
    reserve: bool,
}
impl<ARA: AsyncReadAncillary<AB> + ?Sized, AB: CmsgMut + ?Sized> AsyncRead for ReadAncillaryPartAppl<'_, '_, ARA, AB> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        if self.reserve {
            let _ = self.abuf.reserve_up_to_exact(buf.len());
        }
        let Self { slf, abuf, ret, .. } = self.get_mut();
        let sc = ready!(Pin::new(slf).poll_read_ancillary(cx, buf, abuf))?;
        *ret += sc;
        Poll::Ready(Ok(sc.main))
    }
}

/// [Future] returned by [`read_ancillary_to_end()`](super::AsyncReadAncillaryExt::read_ancillary_to_end) and
/// [`read_to_end_with_ancillary()`](super::AsyncReadAncillaryExt::read_to_end_with_ancillary).
pub struct ReadToEndAncillary<'slf, 'b, 'ab, AB: ?Sized, T: ?Sized> {
    partappl: ReadAncillaryPartAppl<'slf, 'ab, T, AB>,
    buf: &'b mut Vec<u8>,
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'slf, 'b, 'ab, AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadToEndAncillary<'slf, 'b, 'ab, AB, T>
{
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, buf: &'b mut Vec<u8>, abuf: &'ab mut AB, reserve: bool) -> Self {
        Self {
            partappl: ReadAncillaryPartAppl {
                slf: Pin::new(slf),
                abuf,
                ret: Default::default(),
                reserve,
            },
            buf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadToEndAncillary<'_, '_, '_, AB, T>
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
        Poll::Ready(Ok(slf.partappl.ret))
    }
}

/// [Future] returned by [`read_exact_with_ancillary()`](super::AsyncReadAncillaryExt::read_exact_with_ancillary).
pub struct ReadExactWithAncillary<'slf, 'b, 'ab, AB: ?Sized, T: ?Sized> {
    partappl: ReadAncillaryPartAppl<'slf, 'ab, T, AB>,
    buf: &'b mut [u8],
    _phantom: PhantomData<for<'a> fn(&'a mut AB)>,
}
impl<'slf, 'b, 'ab, AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized>
    ReadExactWithAncillary<'slf, 'b, 'ab, AB, T>
{
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, buf: &'b mut [u8], abuf: &'ab mut AB) -> Self {
        Self {
            partappl: ReadAncillaryPartAppl {
                slf: Pin::new(slf),
                abuf,
                ret: Default::default(),
                reserve: false,
            },
            buf,
            _phantom: PhantomData,
        }
    }
}
impl<AB: CmsgMut + ?Sized, T: AsyncReadAncillary<AB> + Unpin + ?Sized> Future
    for ReadExactWithAncillary<'_, '_, '_, AB, T>
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
        Poll::Ready(Ok(slf.partappl.ret))
    }
}

/// [Future] returned by [`write_ancillary()`](super::AsyncWriteAncillaryExt::write_ancillary).
pub struct WriteAncillary<'slf, 'b, 'ab, 'ac, T: ?Sized> {
    slf: &'slf mut T,
    buf: &'b [u8],
    abuf: CmsgRef<'ab, 'ac>,
}
impl<'slf, 'b, 'ab, 'ac, T: AsyncWriteAncillary + Unpin + ?Sized> WriteAncillary<'slf, 'b, 'ab, 'ac, T> {
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, buf: &'b [u8], abuf: CmsgRef<'ab, 'ac>) -> Self {
        Self { slf, buf, abuf }
    }
}
impl<T: AsyncWriteAncillary + Unpin + ?Sized> Future for WriteAncillary<'_, '_, '_, '_, T> {
    type Output = io::Result<usize>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_write_ancillary(cx, slf.buf, slf.abuf)
    }
}

/// [Future] returned by [`write_ancillary_vectored()`](super::AsyncWriteAncillaryExt::write_ancillary_vectored).
pub struct WriteAncillaryVectored<'slf, 'b, 'iov, 'ab, 'ac, T: ?Sized> {
    slf: &'slf mut T,
    bufs: &'b [IoSlice<'iov>],
    abuf: CmsgRef<'ab, 'ac>,
}
impl<'slf, 'b, 'iov, 'ab, 'ac, T: AsyncWriteAncillary + Unpin + ?Sized>
    WriteAncillaryVectored<'slf, 'b, 'iov, 'ab, 'ac, T>
{
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, bufs: &'b [IoSlice<'iov>], abuf: CmsgRef<'ab, 'ac>) -> Self {
        Self { slf, bufs, abuf }
    }
}
impl<T: AsyncWriteAncillary + Unpin + ?Sized> Future for WriteAncillaryVectored<'_, '_, '_, '_, '_, T> {
    type Output = io::Result<usize>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = &mut self.get_mut();
        Pin::new(&mut slf.slf).poll_write_ancillary_vectored(cx, slf.bufs, slf.abuf)
    }
}

//--- Actual adapters ---

struct WriteAncillaryPartAppl<'slf, 'ab, 'ac, AWA: ?Sized> {
    slf: Pin<&'slf mut AWA>,
    abuf: CmsgRef<'ab, 'ac>,
}
// hi myrl
impl<AWA: AsyncWriteAncillary + ?Sized> AsyncWrite for WriteAncillaryPartAppl<'_, '_, '_, AWA> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let Self { slf, abuf } = self.get_mut();
        let pin = Pin::new(slf);
        let bytes_written = if !abuf.inner().is_empty() {
            let bw = ready!(pin.poll_write_ancillary(cx, buf, *abuf))?;
            abuf.consume_bytes(abuf.inner().len());
            bw
        } else {
            ready!(pin.poll_write(cx, buf))?
        };
        Poll::Ready(Ok(bytes_written))
    }
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Self { slf, .. } = self.get_mut();
        Pin::new(slf).poll_flush(cx)
    }
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Self { slf, .. } = self.get_mut();
        Pin::new(slf).poll_close(cx)
    }
}

/// [Future] returned by [`write_all_ancillary()`](super::AsyncWriteAncillaryExt::write_all_ancillary).
pub struct WriteAllAncillary<'slf, 'b, 'ab, 'ac, T: ?Sized> {
    partappl: WriteAncillaryPartAppl<'slf, 'ab, 'ac, T>,
    buf: &'b [u8],
}
impl<'slf, 'b, 'ab, 'ac, T: AsyncWriteAncillary + Unpin + ?Sized> WriteAllAncillary<'slf, 'b, 'ab, 'ac, T> {
    #[inline(always)]
    pub(super) fn new(slf: &'slf mut T, buf: &'b [u8], abuf: CmsgRef<'ab, 'ac>) -> Self {
        Self {
            partappl: WriteAncillaryPartAppl {
                slf: Pin::new(slf),
                abuf,
            },
            buf,
        }
    }
}
impl<T: AsyncWriteAncillary + ?Sized> Future for WriteAllAncillary<'_, '_, '_, '_, T> {
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
