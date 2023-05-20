//! Socket control message manipulation.
//!
//! This module contains a generic safe framework for control messages – not just for Unix domain sockets, but for any `recvmsg`/`sendmsg`-based API in the Unix socket architecture.
//!
//! The [`ancillary`] module contains safe wrappers that can help you correctly initialize and parse ancillary data control messages; its types can then be fed into [`CmsgBuffer`] or [`CmsgMut`] via the `.add_message()` method on both of those structs.
// TODO parser

pub mod ancillary;

mod buffer;
mod mmut;
mod mref;

pub use {buffer::*, mmut::*, mref::*};

use libc::{c_int, c_uint, msghdr, CMSG_SPACE};
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    ops::Deref,
};

/// A **c**ontrol **m**e**s**sa**g**e, consisting of a level, type and its payload.
///
/// The type encodes the memory safety of the control message with the specified level, type and payload being sent, in the form of a safety guarantee. It intentionally does not implement [`Copy`] and [`Clone`] because control messages, as exemplified by [`ancillary::FileDescriptors`], can transfer ownership over resources, which requires that only move semantics be provided.
#[derive(Debug, PartialEq, Eq)]
pub struct Cmsg<'a> {
    cmsg_level: c_int,
    cmsg_type: c_int,
    data: &'a [u8],
    // TODO truncation flag
}
impl<'a> Cmsg<'a> {
    /// Constructs a control message with the given level, type and payload.
    ///
    /// # Safety
    /// - The length of `data` must not exceed the maximum value of `c_uint`.
    /// - The contents of `data` are not checked with respect to the supplied `cmsg_level` and cmsg_type`, which means that OS-specific functionality invoked via ancillary data cannot be accounted for or validated. For example, passing file descriptors in this way can invalidate Rust RAII ownership of them as resources, effectively violating the contract of `FromRawFd` via the use of `AsRawFd`.
    #[inline(always)]
    pub const unsafe fn new(cmsg_level: c_int, cmsg_type: c_int, data: &'a [u8]) -> Self {
        assert!(
            data.len() <= c_uint::MAX as usize,
            "length of payload does not fit in c_uint"
        );
        Self {
            cmsg_level,
            cmsg_type,
            data,
        }
    }
    /// Returns the `cmsg_level` of the control message.
    #[inline(always)]
    pub const fn cmsg_level(&self) -> c_int {
        self.cmsg_level
    }
    /// Returns the `cmsg_type` of the control message.
    #[inline(always)]
    pub const fn cmsg_type(&self) -> c_int {
        self.cmsg_type
    }
    /// Returns the payload of the control message.
    #[inline(always)]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }
    /// Returns the amount of space the control message occupies in a control message buffer.
    #[inline(always)]
    pub fn space_occupied(&self) -> usize {
        unsafe { CMSG_SPACE(self.data.len() as c_uint) as usize }
    }
    /// Clones the control message. No special treatment of the contained data is performed, and the struct is simply copied bitwise, with the data slice pointing to the same memory.
    ///
    /// # Safety
    /// As outlined in the [struct-level documentation](Cmsg), control messages can potentially and unknowingly have ownership over resources (such as [file descriptors](ancillary::FileDescriptors)), which means that cloning the raw control message and then parsing it twice can lead to a double-free scenario. This method should only be called if the original copy then never gets parsed, is known to not own any resources as per `cmsg_level` and `cmsg_type` or if the potential unsafe double-free outcome is averted by some other means.
    #[inline(always)]
    pub const unsafe fn clone_unchecked(&self) -> Self {
        Self {
            cmsg_level: self.cmsg_level,
            cmsg_type: self.cmsg_type,
            data: self.data,
        }
    }
}

/// The error type for the construction of [`CmsgMut`] from a slice, indicating that the slice size overflowed `isize`.
pub struct BufferTooBig<T: Deref<Target = [E]>, E>(T);
impl<T: Deref<Target = [E]>, E> Debug for BufferTooBig<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let buf = self.0.deref();
        f.debug_struct("BufferTooBig")
            .field("base", &buf.as_ptr())
            .field("length", &buf.len())
            .finish()
    }
}
impl<T: Deref<Target = [E]>, E> Display for BufferTooBig<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "buffer with length {} overflowed `isize`", self.0.deref().len())
    }
}
impl<T: Deref<Target = [E]>, E> Error for BufferTooBig<T, E> {}
