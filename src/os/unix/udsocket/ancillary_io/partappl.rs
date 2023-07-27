use super::ReadAncillarySuccess;
use crate::os::unix::{
    udsocket::{
        cmsg::{CmsgMut, CmsgMutExt, CmsgRef},
        UdSocket,
    },
    unixprelude::*,
};

// TODO document pin behavior

/// An adapter from [`WriteAncillary`] to [`Write`] that
/// [partially applies](https://en.wikipedia.org/wiki/Partial_application) the former and allows the use of further
/// adapters described in terms of the latter.
///
/// This struct is primarily created by [`WriteAncillaryExt::with_cmsg_ref()`] (or its by-value variant), although it is
/// perfectly fine to construct it manually, as all its fields are public. See the documentation of the above method for
/// more details on how this adapter works.
#[derive(Copy, Clone, Debug, Default)]
pub struct WithCmsgRef<'abuf, WA> {
    /// The writer whose [`WriteAncillary`] implementation is to be the delegation target of this adapter's [`Write`].
    pub writer: WA,
    /// The ancillary data to be passed to `writer`. It will be completely consumed after the first call unless it is
    /// explicitly replenished later.
    pub abuf: CmsgRef<'abuf>,
}
impl<WA> WithCmsgRef<'_, WA> {
    /// Unwraps the adapter, returning the original writer.
    #[inline(always)]
    pub fn into_inner(self) -> WA {
        self.writer
    }
}
impl<WA: AsFd> AsFd for WithCmsgRef<'_, WA> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.writer.as_fd()
    }
}
impl<WA: UdSocket> UdSocket for WithCmsgRef<'_, WA> {}

/// An adapter from [`ReadAncillary`] to [`Write`] that
/// [partially applies](https://en.wikipedia.org/wiki/Partial_application) the former and allows the use of further
/// adapters described in terms of the latter.
///
/// This struct is produced by [`ReadAncillaryExt::with_cmsg_mut()`]. See its documentation for more.
#[derive(Debug)]
pub struct WithCmsgMut<'abuf, RA, AB: ?Sized> {
    /// The reader whose [`ReadAncillary`] implementation is to be the delegation target of this adapter's [`Read`].
    pub reader: RA,
    /// The ancillary data buffer to be passed to `reader` to read control messages into.
    pub abuf: &'abuf mut AB,
    /// Whether the ancillary buffer is to be resized to match or exceed the passed main-band buffer size on every read.
    /// Note that not all implementations of [`CmsgMut`] support this behavior, and that some will silently ignore all
    /// resize attempts.
    ///
    /// Defaults to `true`.
    pub reserve: bool,
    pub(super) accumulator: ReadAncillarySuccess,
}
impl<RA, AB: ?Sized> WithCmsgMut<'_, RA, AB> {
    /// Returns how much regular data and ancillary data has been read (both in bytes).
    #[inline(always)]
    pub fn total_read(&self) -> ReadAncillarySuccess {
        self.accumulator
    }
    /// Unwraps the adapter, returning the original reader.
    #[inline(always)]
    pub fn into_inner(self) -> RA {
        self.reader
    }
}
impl<'abuf, RA, AB: CmsgMut + ?Sized> WithCmsgMut<'abuf, RA, AB> {
    #[inline(always)]
    pub(super) fn new(reader: RA, abuf: &'abuf mut AB) -> Self {
        Self {
            reader,
            abuf,
            reserve: true,
            accumulator: ReadAncillarySuccess { main: 0, ancillary: 0 },
        }
    }
    pub(super) fn maybe_reserve(&mut self, buflen: usize) {
        if self.reserve {
            // The `exact` variant is used here because the various utility methods from the standard library have their
            // own clever decision-making for the buffer size and we don't want the ancillary buffer to perform
            // guesswork which the main buffer has already performed by the time this function gets called. The result
            // we ignore because not resizing is intended behavior for when it's impossible.
            let _ = self.abuf.reserve_up_to_exact(buflen);
        }
    }
}
impl<RA: AsFd, AB: ?Sized> AsFd for WithCmsgMut<'_, RA, AB> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.reader.as_fd()
    }
}
impl<RA: UdSocket, AB: ?Sized> UdSocket for WithCmsgMut<'_, RA, AB> {}
