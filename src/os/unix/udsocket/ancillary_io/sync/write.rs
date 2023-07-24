use super::super::devector;
use crate::os::unix::udsocket::{cmsg::*, WithCmsgRef};
use std::{
    fmt::Arguments,
    io::{self, prelude::*, IoSlice},
};

/// An extension of [`Write`] that enables operations involving ancillary data.
pub trait WriteAncillary: Write {
    /// Analogous to [`Write::write()`], but also sends control messages from the given ancillary buffer.
    ///
    /// The return value only the amount of main-band data sent from the given regular buffer – the entirety of the
    /// given `abuf` is always sent in full.
    fn write_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_, '_>) -> io::Result<usize>;

    /// Same as [`.write_ancillary()`](WriteAncillary::write_ancillary), but performs a
    /// [gather write](https://en.wikipedia.org/wiki/Vectored_I%2FO) instead.
    fn write_ancillary_vectored(&mut self, bufs: &[IoSlice<'_>], abuf: CmsgRef<'_, '_>) -> io::Result<usize> {
        self.write_ancillary(devector(bufs), abuf)
    }
}

impl<T: WriteAncillary + ?Sized> WriteAncillary for &mut T {
    forward_trait_method!(
        fn write_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_, '_>)
            -> io::Result<usize>
    );
    forward_trait_method!(
        fn write_ancillary_vectored(
            &mut self,
            bufs: &[IoSlice<'_>],
            abuf: CmsgRef<'_, '_>,
        ) -> io::Result<usize>
    );
}
impl<T: WriteAncillary + ?Sized> WriteAncillary for Box<T> {
    forward_trait_method!(
        fn write_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_, '_>)
            -> io::Result<usize>
    );
    forward_trait_method!(
        fn write_ancillary_vectored(
            &mut self,
            bufs: &[IoSlice<'_>],
            abuf: CmsgRef<'_, '_>,
        ) -> io::Result<usize>
    );
}
pub(crate) fn write_in_terms_of_vectored(
    slf: &mut impl WriteAncillary,
    buf: &[u8],
    abuf: CmsgRef<'_, '_>,
) -> io::Result<usize> {
    slf.write_ancillary_vectored(&[IoSlice::new(buf)], abuf)
}

#[cfg(debug_assertions)]
fn _assert_ext<T: WriteAncillaryExt>(x: T) -> T {
    x
}
#[cfg(debug_assertions)]
fn _assert_write_ancillary_object_safe<'a, T: WriteAncillary + 'a>(x: &mut T) -> &mut (dyn WriteAncillary + 'a) {
    _assert_ext(x)
}

/// Methods derived from the interface of [`WriteAncillary`].
pub trait WriteAncillaryExt: WriteAncillary {
    /// Mutably borrows the writer and returns an adapter from [`WriteAncillary`] to [`Write`] that
    /// [partially applies](https://en.wikipedia.org/wiki/Partial_application) the former and allows the use of further
    /// adapters described in terms of the latter.
    ///
    /// The adapter stores a [`CmsgRef`] with ancillary data to be sent on the first call to [`.write()`](Write::write),
    /// via delegation to [`.write_ancillary()`](WriteAncillary::write_ancillary). This allows sending ancillary data
    /// through I/O utilities that are completely oblivious to its existence, transforming a `WriteAncillary` interface
    /// into that of `Write` while retaining the ancillary sending behavior.
    ///
    /// # Notes
    ///
    /// - Since the ancillary data is sent in its entirety on the first write operation, further ones will have no
    /// ancillary data to send unless it gets explicitly updated by the caller.
    ///
    /// - This adapter will always optimize out the call to `.write_ancillary()` and simply delegate to the inner
    /// writer's `.write()` if the stored `abuf` is empty (has a length of zero).
    ///
    /// - Even though all implementors of `WriteAncillary` are necessarily implementors of `Write`, their
    /// implementation of `Write` is normally different from that of this type, as they would simply send no ancillary
    /// data since none is provided.
    #[inline(always)]
    fn with_cmsg_ref<'writer, 'b, 'c>(
        &'writer mut self,
        abuf: CmsgRef<'b, 'c>,
    ) -> WithCmsgRef<'b, 'c, &'writer mut Self> {
        WriteAncillaryExt::with_cmsg_ref_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_ref()`](WriteAncillaryExt::with_cmsg_ref), but does not borrow `self`, consuming ownership
    /// instead.
    #[inline(always)]
    fn with_cmsg_ref_by_val<'b, 'c>(self, abuf: CmsgRef<'b, 'c>) -> WithCmsgRef<'b, 'c, Self>
    where
        Self: Sized,
    {
        WithCmsgRef { writer: self, abuf }
    }

    /// Analogous to [`.write_all()`](Write::write_all), but also writes ancillary data.
    #[inline]
    fn write_all_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_, '_>) -> io::Result<()> {
        self.with_cmsg_ref(abuf).write_all(buf)
    }

    /// Analogous to [`.write_fmt()`](Write::write_fmt), but also writes ancillary data.
    #[inline]
    fn write_fmt_ancillary(&mut self, fmt: Arguments<'_>, abuf: CmsgRef<'_, '_>) -> io::Result<()> {
        self.with_cmsg_ref(abuf).write_fmt(fmt)
    }
}
impl<T: WriteAncillary + ?Sized> WriteAncillaryExt for T {}

impl<WA: WriteAncillary> Write for WithCmsgRef<'_, '_, WA> {
    /// Writes via [`.write_ancillary()`](WriteAncillary::write_ancillary) of the inner writer with the `abuf`
    /// argument being `self.abuf`; if `abuf` is empty, [`.write()`](Write::write) of the inner writer is simply used.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = if !self.abuf.inner().is_empty() {
            let bw = self.writer.write_ancillary(buf, self.abuf)?;
            self.abuf.consume_bytes(self.abuf.inner().len());
            bw
        } else {
            self.writer.write(buf)?
        };
        Ok(bytes_written)
    }

    /// Flushes the inner writer, which normally does nothing, since sockets can't be flushed.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Writes via [`.write_ancillary_vectored()`](WriteAncillary::write_ancillary_vectored) of the inner writer with
    /// the `abuf` argument being `self.abuf`; if `abuf` is empty, [`.write_vectored()`](Write::write_vectored) of the
    /// inner writer is simply used.
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let bytes_written = if !self.abuf.inner().is_empty() {
            let bw = self.writer.write_ancillary_vectored(bufs, self.abuf)?;
            self.abuf.consume_bytes(self.abuf.inner().len());
            bw
        } else {
            self.writer.write_vectored(bufs)?
        };
        Ok(bytes_written)
    }

    /// Performs one write via [`.write_ancillary()`](WriteAncillary::write_ancillary) and the rest via the underlying
    /// writer's own `.write_all()` implementation, eliding the latter call if the first write wrote everything.
    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        let bytes_written = self.write(buf)?;
        buf = &buf[bytes_written..];
        if !buf.is_empty() {
            self.writer.write_all(buf)?;
        }
        Ok(())
    }

    // FUTURE is_vectored, write_all_vectored
}
