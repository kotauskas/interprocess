use super::{ancillary::ToCmsg, context::DummyCollector, *};
use std::{mem::MaybeUninit, slice};

/// A **c**ontrol **m**e**s**sa**g**e buffer, used to store the encoded form of ancillary data.
pub struct CmsgBuffer<C = DummyCollector> {
    buf: Vec<u8>,
    /// The context collector stored alongside the buffer.
    ///
    /// `.as_ref()` and `.as_mut()` borrow this field (immutably and mutably, respectively) for decoding and context
    /// collection respectively.
    pub context_collector: C,
}
impl CmsgBuffer {
    /// Creates a buffer with the specified capacity. Using a capacity of 0 makes for a useless buffer, but does not
    /// allocate.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self::from_buf(Vec::with_capacity(capacity))
    }
    /// Converts a `Vec<u8>` to a `CmsgBuffer`, discarding all its data in the process.
    #[inline]
    pub fn from_buf(buf: Vec<u8>) -> Self {
        Self::from_buf_with_collector(buf, DummyCollector)
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer will lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        Self {
            buf,
            context_collector: DummyCollector,
        }
    }
}
impl<C> CmsgBuffer<C> {
    /// Creates a buffer with the specified capacity and an owned context collector. Using a capacity of 0 makes for a
    /// useless buffer, but does not allocate.
    #[inline]
    pub fn new_with_collector(capacity: usize, context_collector: C) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            context_collector,
        }
    }
    /// Converts a `Vec<u8>` to a `CmsgBuffer`, discarding all its data in the process. The given context collector is
    /// also added into the mix.
    pub fn from_buf_with_collector(mut buf: Vec<u8>, context_collector: C) -> Self {
        buf.clear();
        Self { buf, context_collector }
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    /// The given context collector is also added into the mix.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer will lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_with_collector_unchecked(buf: Vec<u8>, context_collector: C) -> Self {
        Self { buf, context_collector }
    }

    /// Borrows the control message buffer. The resulting type retains the validity guarantee.
    #[inline(always)]
    pub fn as_ref(&self) -> CmsgRef<'_> {
        unsafe {
            // SAFETY: validity guarantee is enforced by CmsgBuffer as well
            CmsgRef::new_unchecked(self.buf.as_ref()).expect("Vec allocation length erroneously overflowed `isize`")
        }
    }
    /// Mutably borrows the control message buffer. The resulting type retains the validity guarantee, but does not feed
    /// the initialization cursor back into the owned buffer object.
    #[inline(always)]
    pub fn as_mut(&mut self) -> CmsgMut<'_, &mut C> {
        // This is unsafe in the public interface, but not for the internals. The non-method borrow is to allow struct
        // fields to be mutably borrowed independently.
        let buf = Self::vec_as_uninit_slice_mut(&mut self.buf);
        CmsgMut::new_with_collector(buf, &mut self.context_collector)
    }

    /// Converts the given message object to a [`Cmsg`] and adds it to the buffer, advances the initialization cursor of
    /// `self` such that the next message, if one is added, will appear after it, and returns how much the cursor was
    /// advanced by (i.e. how many more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// Using the return value isn't strictly necessary – calling `.add_message()` again will correctly add one more
    /// message to the buffer.
    pub fn add_message(&mut self, msg: &impl ToCmsg) -> usize {
        let mut ret = 0;
        msg.add_to_buffer(|cmsg| ret = self.add_raw_message(cmsg));
        ret
    }
    /// Adds the specified control message to the buffer, advances the initialization cursor of `self` such that the
    /// next message, if one is added, will appear after it, and returns how much the cursor was advanced by (i.e. how
    /// many more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// Using the return value isn't strictly necessary – calling `.add_raw_message()` again will correctly add one more
    /// message to the buffer.
    pub fn add_raw_message(&mut self, cmsg: Cmsg<'_>) -> usize {
        self.buf.reserve(cmsg.space_occupied());
        let len = self.buf.len();
        let delta = self.as_mut().add_raw_message(cmsg);
        unsafe {
            // SAFETY: we trust add_raw_message() to initialize that much of our buffer
            self.set_len(len + delta);
        };
        delta
    }

    /// Assumes that the first `len` bytes of the buffer are initialized memory and valid ancillary data.
    ///
    /// # Safety
    /// See [`Vec::set_len()`] and [`Cmsg::new()`].
    pub unsafe fn set_len(&mut self, len: usize) {
        assert!(
            len <= self.buf.capacity(),
            "cannot set initialized length past buffer capacity"
        );
        unsafe {
            self.buf.set_len(len);
        }
    }
    /// Exclusively borrows the whole buffer as a slice of possibly uninitialized bytes.
    ///
    /// # Safety
    /// The contents of the buffer must not be modified in a way which could invalidate the ancillary data contained and
    /// cause undefined behavior via the system C library entering an out-of-bounds condition or otherwise violating the
    /// guarantees of a Rust type.
    #[inline]
    pub unsafe fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        Self::vec_as_uninit_slice_mut(&mut self.buf)
    }
    #[inline]
    fn vec_as_uninit_slice_mut(vec: &mut Vec<u8>) -> &mut [MaybeUninit<u8>] {
        unsafe {
            // SAFETY: we're just turning the whole Vec buffer into a `MaybeUninit` slice here
            slice::from_raw_parts_mut(vec.as_mut_ptr().cast::<MaybeUninit<u8>>(), vec.capacity())
        }
    }
}
impl From<Vec<u8>> for CmsgBuffer {
    #[inline]
    fn from(buf: Vec<u8>) -> Self {
        Self::from_buf(buf)
    }
}
