use super::super::devector;
use crate::os::unix::udsocket::cmsg::*;
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

    /// Same as [`write_ancillary`](WriteAncillary::write_ancillary), but performs a
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
fn _assert_write_ancillary_object_safe<'a, T: WriteAncillary + 'a>(x: &mut T) -> &mut (dyn WriteAncillaryExt + 'a) {
    x
}

/// Methods derived from the interface of [`WriteAncillary`].
pub trait WriteAncillaryExt: WriteAncillary {
    /// Analogous to [`write_all`](Write::write_all), but also writes ancillary data.
    fn write_all_ancillary(&mut self, buf: &[u8], abuf: CmsgRef<'_, '_>) -> io::Result<()> {
        let mut partappl = WriteAncillaryPartAppl { slf: self, abuf };
        partappl.write_all(buf)
    }

    /// Analogous to [`write_fmt`](Write::write_fmt), but also writes ancillary data.
    fn write_fmt_ancillary(&mut self, fmt: Arguments<'_>, abuf: CmsgRef<'_, '_>) -> io::Result<()> {
        let mut partappl = WriteAncillaryPartAppl { slf: self, abuf };
        partappl.write_fmt(fmt)
    }
}
impl<T: WriteAncillary + ?Sized> WriteAncillaryExt for T {}

/// Like `ReadAncillaryPartAppl`, but for `WriteAncillary` instead.
struct WriteAncillaryPartAppl<'slf, 'b, 'c, WA: ?Sized> {
    slf: &'slf mut WA,
    abuf: CmsgRef<'b, 'c>,
}
impl<WA: WriteAncillary + ?Sized> Write for WriteAncillaryPartAppl<'_, '_, '_, WA> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = if !self.abuf.inner().is_empty() {
            let bw = self.slf.write_ancillary(buf, self.abuf)?;
            self.abuf.consume_bytes(self.abuf.inner().len());
            bw
        } else {
            self.slf.write(buf)?
        };
        Ok(bytes_written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.slf.flush()
    }
}
