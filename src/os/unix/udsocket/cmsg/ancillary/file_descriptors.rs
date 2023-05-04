//! [`FileDescriptors`] and associated helper types.
use super::*;
use std::{
    mem::{size_of, transmute},
    os::fd::{BorrowedFd, FromRawFd, OwnedFd, RawFd},
    slice,
};

/// Ancillary data message that allows sending ownership of file descriptors over to another process.
///
/// The file descriptors are stored as a slice of [`OwnedFd`]s.
#[derive(Debug, Default)]
pub struct FileDescriptors<'a>(UnalignedFdSlice<'a>);
impl<'a> FileDescriptors<'a> {
    pub(super) const TYPE: c_int = libc::SCM_RIGHTS;

    /// Constructs the ancillary data message from a slice of [borrowed file descriptors](BorrowedFd).
    ///
    /// The file descriptor lifetime must outlive the slice.
    #[inline]
    pub const fn new(descriptors: &[BorrowedFd<'a>]) -> Self {
        Self(UnalignedFdSlice::from_borrowed_fd_slice(descriptors))
    }
    /// Constructs the ancillary data message from a slice of [raw file descriptors](RawFd). If `owned` is true, they will be dropped together with the whole struct.
    ///
    /// # Safety
    /// The file descriptors must be valid. If `owned` is true, there must not be another owner.
    #[inline]
    pub const unsafe fn new_raw(descriptors: &'a [RawFd], owned: bool) -> Self {
        unsafe { Self(UnalignedFdSlice::from_raw_fd_slice(descriptors, owned)) }
    }
}
impl ToCmsg for FileDescriptors<'_> {
    fn add_to_buffer(&self, add_fn: impl FnOnce(Cmsg<'_>)) {
        let cmsg = unsafe {
            // SAFETY: a bunch of file descriptors is all you need for a SCM_RIGHTS control message
            Cmsg::new(LEVEL, Self::TYPE, self.0.as_bytes())
        };

        add_fn(cmsg);
    }
}
impl<'a> FromCmsg<'a> for FileDescriptors<'a> {
    type MalformedPayloadError = Infallible;

    fn try_parse(cmsg: Cmsg<'a>) -> ParseResult<'a, Self, Self::MalformedPayloadError> {
        use ParseErrorKind::*;
        let (lvl, ty) = (cmsg.cmsg_level(), cmsg.cmsg_type());
        if lvl != LEVEL {
            return Err(WrongLevel {
                expected: Some(LEVEL),
                got: lvl,
            }
            .wrap(cmsg));
        }
        if ty != Self::TYPE {
            return Err(WrongType {
                expected: Some(Self::TYPE),
                got: ty,
            }
            .wrap(cmsg));
        }

        unsafe {
            // SAFETY: we trust the Linux kernel, don't we? Also, that Cmsg isn't `Copy` or `Clone` or anything, so we
            // can safely own these descriptors.
            Ok(Self(UnalignedFdSlice::from_byte_slice(cmsg.data(), true)))
        }
    }
}

type UnalignedFdBytes = [u8; size_of::<RawFd>()];
/// Unaligned file descriptor with an initialization guarantee.
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
struct UnalignedFd(UnalignedFdBytes);
impl UnalignedFd {
    #[inline(always)]
    const fn to_raw(self) -> RawFd {
        RawFd::from_ne_bytes(self.0)
    }
    /// Converts to [`OwnedFd`].
    ///
    /// # Safety
    /// Assumes that the file descriptor isn't owned by anything else.
    unsafe fn into_owned_fd(self) -> OwnedFd {
        unsafe {
            // SAFETY: same safety guarantee of contained value
            OwnedFd::from_raw_fd(c_int::from_ne_bytes(self.0))
        }
    }
}
impl Debug for UnalignedFd {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.to_raw(), f)
    }
}

/// Structure that allows itself to have the unique ability to own stuff by immutable reference. Crazy, right?
#[derive(Debug, Default)]
struct UnalignedFdSlice<'a> {
    fds: &'a [UnalignedFd],
    owned: bool,
}
impl<'a> UnalignedFdSlice<'a> {
    /// Performs slice reference-to-reference conversion from raw file descriptors.
    ///
    /// # Safety
    /// Akin to `FromRawFd`.
    const unsafe fn from_raw_fd_slice(fds: &[RawFd], owned: bool) -> Self {
        let fds = unsafe {
            // SAFETY: size is same, alignment is less strict
            transmute::<&[RawFd], &[UnalignedFd]>(fds)
        };
        Self { fds, owned }
    }
    /// Performs slice reference-to-reference conversion from borrowed file descriptors.
    const fn from_borrowed_fd_slice(fds: &[BorrowedFd<'a>]) -> Self {
        let fds = unsafe {
            // SAFETY: size is same, alignment is less strict
            transmute::<&[BorrowedFd<'a>], &[RawFd]>(fds)
        };
        unsafe {
            // SAFETY: that's what BorrowedFd is for
            Self::from_raw_fd_slice(fds, false)
        }
    }
    /// Performs slice reference-to-reference conversion from bytes.
    ///
    /// # Safety
    /// Akin to `FromRawFd`.
    const unsafe fn from_byte_slice(bytes: &[u8], owned: bool) -> Self {
        let (ptr, size_bytes) = (bytes.as_ptr(), bytes.len());
        let size_fds = size_bytes / size_of::<UnalignedFdBytes>();
        let fds = unsafe {
            // SAFETY: the two are layout-compatible, byte alignment is the exact same
            slice::from_raw_parts(ptr.cast::<UnalignedFd>(), size_fds)
        };
        Self { fds, owned }
    }

    const fn as_bytes(&self) -> &[u8] {
        let (ptr, size_fds) = (self.fds.as_ptr(), self.fds.len());
        let size_bytes = size_fds * size_of::<UnalignedFdBytes>();
        unsafe {
            // SAFETY: the two are layout-compatible, byte alignment is the exact same
            slice::from_raw_parts(ptr.cast::<u8>(), size_bytes)
        }
    }
}
impl Drop for UnalignedFdSlice<'_> {
    fn drop(&mut self) {
        if self.owned {
            for fd in self.fds {
                let _ = unsafe {
                    // SAFETY: the owned flag doesn't lie
                    fd.into_owned_fd()
                };
            }
        }
    }
}
