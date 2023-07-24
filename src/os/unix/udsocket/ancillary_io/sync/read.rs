use super::super::devector_mut;
use crate::os::unix::udsocket::{cmsg::*, ReadAncillarySuccess, WithCmsgMut};
use std::io::{self, prelude::*, IoSliceMut};

/// An extension of [`Read`] that enables operations involving ancillary data.
///
/// The generic parameter on the trait allows for trait objects to be constructed. Simply substitute [`DynCmsgMut`] or
/// [`DynCmsgMutStatic`] for `AB` to obtain an object-safe `ReadAncillary`.
pub trait ReadAncillary<AB: CmsgMut + ?Sized>: Read {
    /// Analogous to [`Read::read()`], but also reads control messages into the given ancillary buffer.
    ///
    /// The return value contains both the amount of main-band data read into the given regular buffer and the number of
    /// bytes read into the ancillary buffer.
    fn read_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB) -> io::Result<ReadAncillarySuccess>;

    /// Same as [`read_ancillary`](ReadAncillary::read_ancillary), but performs a
    /// [scatter read](https://en.wikipedia.org/wiki/Vectored_I%2FO) instead.
    fn read_ancillary_vectored(
        &mut self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> io::Result<ReadAncillarySuccess> {
        self.read_ancillary(devector_mut(bufs), abuf)
    }
}

pub(crate) fn read_in_terms_of_vectored<AB: CmsgMut + ?Sized>(
    slf: &mut impl ReadAncillary<AB>,
    buf: &mut [u8],
    abuf: &mut AB,
) -> io::Result<ReadAncillarySuccess> {
    slf.read_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf)
}

#[cfg(debug_assertions)]
fn _assert_ext<T: ReadAncillaryExt<AB>, AB: CmsgMut + ?Sized>(x: T) -> T {
    x
}
#[cfg(debug_assertions)]
fn _assert_read_ancillary_object_safe<'j: 'm + 'c, 'm, 'c, T: ReadAncillary<DynCmsgMut<'m, 'c>> + 'j>(
    x: &mut T,
) -> &mut (dyn ReadAncillary<DynCmsgMut<'m, 'c>> + 'j) {
    _assert_ext(x as _)
}

impl<AB: CmsgMut + ?Sized, T: ReadAncillary<AB> + ?Sized> ReadAncillary<AB> for &mut T {
    forward_trait_method!(
        fn read_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB)
            -> io::Result<ReadAncillarySuccess>
    );
    forward_trait_method!(
        fn read_ancillary_vectored(
            &mut self,
            bufs: &mut [IoSliceMut<'_>],
            abuf: &mut AB,
        ) -> io::Result<ReadAncillarySuccess>
    );
}
impl<AB: CmsgMut + ?Sized, T: ReadAncillary<AB> + ?Sized> ReadAncillary<AB> for Box<T> {
    forward_trait_method!(
        fn read_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB)
            -> io::Result<ReadAncillarySuccess>
    );
    forward_trait_method!(
        fn read_ancillary_vectored(
            &mut self,
            bufs: &mut [IoSliceMut<'_>],
            abuf: &mut AB,
        ) -> io::Result<ReadAncillarySuccess>
    );
}

/// Methods derived from the interface of [`ReadAncillary`].
///
/// See the documentation on `ReadAncillary` for notes on why a type parameter is present.
pub trait ReadAncillaryExt<AB: CmsgMut + ?Sized>: ReadAncillary<AB> {
    /// Mutably borrows the reader and returns an adapter from [`ReadAncillary`] to [`Write`] that
    /// [partially applies](https://en.wikipedia.org/wiki/Partial_application) the former and allows the use of further
    /// adapters described in terms of the latter.
    ///
    /// This struct stores a [`CmsgMut`] with ancillary data to be sent on the first call to [`.read()`](Read::read),
    /// via delegation to [`.read_ancillary()`](ReadAncillary::read_ancillary). This allows receiving ancillary data
    /// through I/O utilities that are completely oblivious to its existence, transforming a `ReadAncillary` interface
    /// into that of `Read` while retaining the ancillary data reception behavior.
    ///
    /// A `reserve` flag, defaulting to `true`, allows the provided ancillary buffer to be automatically resized to
    /// match or exceed the size of the `buf` argument passed to `.read()`. Note that [`CmsgMut`] implementations
    /// support this behavior, with unsupporting ones silently ignoring those resize attempts.
    ///
    /// # Notes
    ///
    /// - Even though all implementors of `ReadAncillary` are necessarily implementors of `Read`, their implementation
    /// of `Read` is normally different from that of this type, as they would simply read no ancillary data since no
    /// buffer is provided.
    #[inline(always)]
    fn with_cmsg_mut<'reader, 'abuf>(
        &'reader mut self,
        abuf: &'abuf mut AB,
    ) -> WithCmsgMut<'abuf, &'reader mut Self, AB> {
        ReadAncillaryExt::with_cmsg_mut_by_val(self, abuf)
    }
    /// Like [`.with_cmsg_mut()`](ReadAncillaryExt::with_cmsg_mut), but does not borrow `self`, consuming ownership
    /// instead.
    #[inline(always)]
    fn with_cmsg_mut_by_val(self, abuf: &mut AB) -> WithCmsgMut<'_, Self, AB>
    where
        Self: Sized,
    {
        WithCmsgMut::new(self, abuf)
    }

    /// Analogous to [`Read::read_to_end()`], but also reads ancillary data into the given ancillary buffer, growing it
    /// with the regular data buffer.
    ///
    /// **Read-to-end semantics apply to both main and ancillary data**, unlike with [`read_to_end_with_ancillary()`],
    /// which only grows the main data buffer and reads ancillary data exactly the same way as a regular
    /// [`read_ancillary`](ReadAncillary::read_ancillary) operation would.
    ///
    /// Note that using a buffer type that doesn't support resizing, such as [`CmsgMutBuf`], will produce identical
    /// behavior to [`read_to_end_with_ancillary()`].
    ///
    /// [`read_to_end_with_ancillary()`]: ReadAncillaryExt::read_to_end_with_ancillary
    fn read_ancillary_to_end(&mut self, buf: &mut Vec<u8>, abuf: &mut AB) -> io::Result<ReadAncillarySuccess> {
        let mut partappl = self.with_cmsg_mut(abuf);
        partappl.read_to_end(buf)?;
        Ok(partappl.total_read())
    }

    /// Analogous to [`Read::read_to_end()`], but also reads ancillary data into the given ancillary buffer.
    ///
    /// **Read-to-end semantics apply only to the main data**, unlike with
    /// [`read_ancillary_to_end()`](ReadAncillaryExt::read_ancillary_to_end), which grows both buffers adaptively and
    /// thus requires both of them to be passed with ownership.
    fn read_to_end_with_ancillary(&mut self, buf: &mut Vec<u8>, abuf: &mut AB) -> io::Result<ReadAncillarySuccess> {
        let mut partappl = self.with_cmsg_mut(abuf);
        partappl.read_to_end(buf)?;
        Ok(partappl.total_read())
    }

    /// Analogous to [`Read::read_exact`], but also reads ancillary data into the given buffer.
    fn read_exact_with_ancillary(&mut self, buf: &mut [u8], abuf: &mut AB) -> io::Result<ReadAncillarySuccess> {
        let mut partappl = self.with_cmsg_mut(abuf);
        partappl.read_exact(buf)?;
        Ok(partappl.total_read())
    }
}
impl<AB: CmsgMut + ?Sized, T: ReadAncillary<AB> + ?Sized> ReadAncillaryExt<AB> for T {}

impl<RA: ReadAncillary<AB>, AB: CmsgMut + ?Sized> Read for WithCmsgMut<'_, RA, AB> {
    /// Reads via [`.read_ancillary()`](ReadAncillary::read_ancillary) on the inner reader with the `abuf` argument
    /// being `self.abuf`.
    ///
    /// If `reserve` is enabled, it will be resized to match or exceed the size of `buf` (if
    /// possible) via [`.reserve_up_to_exact()`](CmsgMutExt::reserve_up_to_exact).
    ///
    /// Only the amount of data read into `buf` is returned, with the amount of ancillary data read being stored in the
    /// adapter to be later retrieved via [`.total_read()`](WithCmsgMut::total_read).
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.maybe_reserve(buf.len());
        let sc = self.reader.read_ancillary(buf, self.abuf)?;
        self.accumulator += sc;
        Ok(sc.main)
    }

    /// Reads via [`.read_ancillary_vectored()`](ReadAncillary::read_ancillary_vectored) on the inner reader with the
    /// `abuf` argument being `self.abuf`.
    ///
    /// If `reserve` is enabled, it will be resized to match or exceed the size of the ***last*** buffer in `bufs` (if
    /// possible) via [`.reserve_up_to_exact()`](CmsgMutExt::reserve_up_to_exact).
    ///
    /// Only the amount of data read into `bufs` is returned, with the amount of ancillary data read being stored in the
    /// adapter to be later retrieved via [`.total_read()`](WithCmsgMut::total_read).
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        if let Some(s) = bufs.last() {
            self.maybe_reserve(s.len());
        }
        let sc = self.reader.read_ancillary_vectored(bufs, self.abuf)?;
        self.accumulator += sc;
        Ok(sc.main)
    }

    // FUTURE is_read_vectored, read_buf
}
