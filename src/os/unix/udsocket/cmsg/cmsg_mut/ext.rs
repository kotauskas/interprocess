use super::{ancillary::ToCmsg, *};
use std::{mem::MaybeUninit, slice};

/// Methods derived from the interface of [`CmsgMut`].
///
/// They're provided in the form of an extension trait to simplify the formulation of safety contracts and guarantees on
/// those methods and on the `CmsgMut` trait itself.
pub trait CmsgMutExt: CmsgMut {
    /// Adds the specified control message to the buffer, advances the validity cursor of `self` such that the next
    /// message, if one is added, will appear after it, and returns how much the cursor was advanced by (i.e. how many
    /// more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// If there isn't enough space, 0 is returned and no message is added. The current implementation will still
    /// introduce padding into the buffer such that `uninit_part()`'s beginning would be well-aligned for `cmsghdr`
    /// even if the size check fails.
    #[inline(always)]
    fn add_raw_message(&mut self, cmsg: Cmsg<'_>) -> usize {
        add_raw::add_raw_message(self, cmsg)
    }
    /// Converts the given message object to a [`Cmsg`] and adds it to the buffer, advances the initialization cursor of
    /// `self` such that the next message, if one is added, will appear after it, and returns how much the cursor was
    /// advanced by (i.e. how many more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// If there isn't enough space, 0 is returned. Peculiarities in this failure case are the same as with
    /// `add_raw_message()`.
    #[inline(always)]
    fn add_message(&mut self, msg: &impl ToCmsg) -> usize {
        self.add_raw_message(msg.to_cmsg())
    }
    /// Returns the capacity of the buffer, which is simply the length of the slice returned by `as_bytes()`.
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.as_bytes().len()
    }
    /// Immutably borrows the part of the buffer which is already filled with valid ancillary data as a [`CmsgRef`].
    ///
    /// Use this method to deserialize the contents of a `CmsgMut` used for receiving control messages from a socket.
    #[inline(always)]
    fn as_ref(&self) -> CmsgRef<'_, '_, Self::Context> {
        unsafe { CmsgRef::new_unchecked_with_context(self.valid_part(), self.context()) }
    }
    /// Immutably borrows the part of the buffer which is already filled with valid ancillary data as a raw slice.
    #[inline(always)]
    fn valid_part(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.as_bytes().as_ptr().cast::<u8>(), self.valid_len()) }
    }
    /// Mutably borrows the part of the buffer which is considered to be uninitialized and/or filled with invalid
    /// ancillary data.
    fn uninit_part(&mut self) -> &mut [MaybeUninit<u8>] {
        let cr = self.as_bytes().as_ptr_range();
        let (buf_start, buf_end) = (cr.start.cast_mut(), cr.end.cast_mut());
        let start = unsafe {
            // SAFETY: as per trait-level contract
            buf_start.add(self.valid_len())
        };
        let len = unsafe { buf_end.offset_from(start) };
        debug_assert!(len >= 0);
        unsafe { slice::from_raw_parts_mut(start, len as usize) }
    }
    /// Splits the buffer into two parts: the left one which is valid and well-initialized and the right one which is
    /// uninitialized, according to `valid_len()`. Only the right part is given as a slice with mutable access; the left
    /// part is given as a [`CmsgRef`].
    ///
    /// This method is useful for deserializing the valid part of the buffer while being able to modify the
    /// uninitialized part.
    fn split_at_init(&mut self) -> (CmsgRef<'_, '_, Self::Context>, &mut [MaybeUninit<u8>]) {
        let ctx: *const _ = self.context();
        let (left, right) = self.raw_split_at_init();
        (
            unsafe {
                // SAFETY: the buffer is valid as per the omnipresent guarantees of `CmsgMut`; the `ctx` borrow is safe
                // because it is contractually required to not overlap with the buffer.
                CmsgRef::new_unchecked_with_context(left, &*ctx)
            },
            right,
        )
    }
    /// Splits the buffer into two parts: the left one which is valid and well-initialized and the right one which is
    /// uninitialized, according to `valid_len()`. Only the right part is given with mutable access. Both parts are
    /// given as raw byte slices.
    fn raw_split_at_init(&mut self) -> (&[u8], &mut [MaybeUninit<u8>]) {
        let (left_base, left_len) = (self.as_bytes().as_ptr(), self.valid_len());
        (
            unsafe {
                // SAFETY: the slice does not overlap with the uninit part and is derived from what was already a valid
                // slice moments ago
                slice::from_raw_parts(left_base.cast::<u8>(), left_len)
            },
            self.uninit_part(),
        )
    }
    /// Splits the buffer into two parts: the left one which is valid and well-initialized and the right one which is
    /// uninitialized, according to `valid_len()`. Both are returned as raw slices with mutable access, which must be
    /// used with care.
    ///
    /// # Safety
    /// The validity of the initialized part must not be compromised by the caller.
    unsafe fn raw_split_at_init_mut(&mut self) -> (&mut [u8], &mut [MaybeUninit<u8>]) {
        let (left_base, left_len) = (self.as_bytes().as_ptr().cast_mut(), self.valid_len());
        (
            unsafe {
                // SAFETY: the slice does not overlap with the uninit part and is derived from what was already a valid
                // slice moments ago
                slice::from_raw_parts_mut(left_base.cast::<u8>(), left_len)
            },
            self.uninit_part(),
        )
    }
    /// Alias for `set_len(valid_len() + incr)`.
    ///
    /// # Safety
    /// See `set_len()`.
    #[inline(always)]
    unsafe fn add_len(&mut self, incr: usize) {
        unsafe {
            // SAFETY: see contract
            self.set_len(self.valid_len() + incr)
        }
    }
    /// Allocates additional space in the buffer via `reserve()` such that its total capacity (counting both existing
    /// capacity and the amount by which the buffer will be grown) reaches or exceeds the given value, at the underlying
    /// data structure's discretion or due to the buffer already being large enough.
    #[inline]
    fn reserve_up_to(&mut self, target: usize) -> ReserveResult {
        let additional = target.saturating_sub(self.capacity());
        if additional != 0 {
            self.reserve(additional)
        } else {
            Ok(())
        }
    }
    /// Like `reserve_up_to()`, but uses `reserve_exact()` instead of `reserve()`
    #[inline]
    fn reserve_up_to_exact(&mut self, target: usize) -> ReserveResult {
        let additional = target.saturating_sub(self.capacity());
        if additional != 0 {
            self.reserve_exact(additional)
        } else {
            Ok(())
        }
    }
}
impl<T: CmsgMut + ?Sized> CmsgMutExt for T {}
