use crate::os::unix::udsocket::util::to_cmsghdr_len;

use super::{
    super::util::{to_msghdr_controllen, DUMMY_MSGHDR},
    ancillary::ToCmsg,
    *,
};
use libc::{c_char, c_int, c_uint, c_void, cmsghdr, msghdr, CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_NXTHDR};
use std::{
    io,
    mem::{size_of, transmute, zeroed, MaybeUninit},
    num::NonZeroUsize,
    ptr, slice,
};

/// A mutable reference to a control message buffer that allows for insertion of ancillary data messages.
#[derive(Debug)]
pub struct CmsgMut<'a> {
    // TODO idea for how to fix the sockcred/cmsgcred debacle (aliasing of SCM_CREDS for two types of struct): add an
    // "interpretation context" field that stores necessary state (such as the value of `LOCAL_CREDS`) to disambiguate
    // without straying too far from what the manpage says is permissible
    buf: &'a mut [MaybeUninit<u8>],
    init_len: usize,
    cmsghdr_offset: Option<NonZeroUsize>,
}
impl<'a> CmsgMut<'a> {
    /// Creates a control message buffer from the given uninitialized slice.
    ///
    /// # Panics
    /// The buffer's length must not overflow `isize`.
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> Self {
        // TODO check against real type of controllen and not isize
        Self::try_from(buf).expect("buffer size overflowed `isize`")
    }

    /// Immutably borrows the initialized part of the control message buffer.
    pub fn as_ref(&self) -> CmsgRef<'_> {
        let init_part = &self.buf[..self.init_len];
        let immslc = unsafe {
            // SAFETY: the init cursor doesn't lie, does it?
            transmute::<&[MaybeUninit<u8>], &[u8]>(init_part)
        };
        unsafe {
            // SAFETY: the validity guarantee is that `add_raw_message()` is correctly implemented and that its input
            // is validated by the unsafe Cmsg factory function. As for the `.unwrap_unchecked()`, that's a superfluous
            // check for what we're already ensuring in `CmsgMut::new()`.
            CmsgRef::new_unchecked(immslc).unwrap_unchecked()
        }
    }

    /// Assumes that the first `len` bytes of the buffer are well-initialized and valid control message buffer data.
    ///
    /// # Safety
    /// The first `len` bytes of the buffer must be well-initialized and valid control message buffer data.
    #[inline]
    pub unsafe fn set_init_len(&mut self, len: usize) {
        debug_assert!(len <= self.buf.len());
        self.init_len = len;
    }
    /// Returns the amount of bytes starting from the beginning of the buffer that are well-initialized and valid control message buffer data.
    #[inline]
    pub fn init_len(&self) -> usize {
        self.init_len
    }

    /// Immutably borrows the buffer, allowing inspection of the underlying data.
    #[inline]
    pub fn inner(&self) -> &[MaybeUninit<u8>] {
        self.buf
    }
    /// Mutably borrows the buffer, allowing arbitrary modifications.
    ///
    /// # Safety
    /// The modifications done to the buffer through the return value, if any, must not amount to invalidation of the control message buffer.
    #[inline]
    pub unsafe fn inner_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.buf
    }

    pub(crate) fn fill_msghdr(&self, hdr: &mut msghdr, full_length: bool) -> io::Result<()> {
        hdr.msg_control = self.buf.as_ptr().cast::<c_void>().cast_mut();
        let len = if full_length { self.buf.len() } else { self.init_len };
        hdr.msg_controllen = to_msghdr_controllen(len)?;
        Ok(())
    }
    fn make_msghdr(&self, full_length: bool) -> msghdr {
        let mut hdr = DUMMY_MSGHDR;
        self.fill_msghdr(&mut hdr, full_length).expect("too big");
        hdr
    }

    /// Finds the first `cmsghdr` with zeroes and returns a reference to it.
    ///
    /// # Safety
    /// `dummy_msghdr` is assumed to be filled in by a prior call of `make_msghdr(true)` on the same bytecrop of `self`.
    unsafe fn first_cmsghdr<'x>(
        buf: &'x mut [MaybeUninit<u8>],
        dummy_msghdr: &msghdr,
    ) -> Option<&'x mut MaybeUninit<cmsghdr>> {
        let origin = buf.as_mut_ptr_range().start;

        if buf.len() < size_of::<cmsghdr>() {
            return None;
        }
        let first_ptr = unsafe { CMSG_FIRSTHDR(dummy_msghdr) }.cast::<MaybeUninit<_>>();
        if first_ptr.is_null() {
            return None;
        }
        debug_assert!(
            first_ptr.cast::<MaybeUninit<u8>>() >= origin,
            "CMSG_FIRSTHDR gave a pointer outside of the buffer"
        );

        unsafe {
            // SAFETY: we trust the implementation of CMSG_FIRSTHDR to do its thing correctly
            Some(&mut *first_ptr)
        }
    }
    /// Fills bytes until the first `cmsghdr` with zeroes and returns a reference to it.
    ///
    /// # Safety
    /// `dummy_msghdr` is assumed to be filled in by a prior call of `make_msghdr(true)` on the same bytecrop of `self`.
    unsafe fn prepare_first_cmsghdr<'x>(
        buf: &'x mut [MaybeUninit<u8>],
        dummy_msghdr: &msghdr,
    ) -> Option<&'x mut MaybeUninit<cmsghdr>> {
        let origin = buf.as_mut_ptr_range().start;

        let first = unsafe {
            // SAFETY: identical safety contract to this function
            Self::first_cmsghdr(buf, dummy_msghdr)?
        };
        let first_bptr = (first as *mut MaybeUninit<cmsghdr>).cast::<MaybeUninit<u8>>();

        if first_bptr > origin {
            unsafe {
                // SAFETY: since the pointer is past the start, finding the offset from the start to it is safe.
                let fill_len = first_bptr.offset_from(origin);
                debug_assert!(fill_len > 0);
                // SAFETY: `start` is within bounds, `bptr` is within bounds, so everything up to `start + fill_len` is
                // within bounds. 0 is a valid value to initialize u8s with. `start` is known to be non-null due to
                // its origin being `.as_mut_ptr_range()`.
                ptr::write_bytes(origin, 0, fill_len as usize);
            }
        }
        Some(first)
    }

    /// Returns a reference to the next `cmsghdr` after the one specified by `offset`.
    ///
    /// # Safety
    /// `offset` must point to a `cmsghdr`, be within `self.buf` and fit into an `isize`.
    unsafe fn next_cmsghdr<'x>(
        buf: &'x mut [MaybeUninit<u8>],
        dummy_msghdr: &msghdr,
        offset: usize,
    ) -> Option<&'x mut MaybeUninit<cmsghdr>> {
        let origin = buf.as_ptr_range().start.cast::<u8>();

        let p_cmsghdr = unsafe {
            // SAFETY: as per safety contract
            origin.add(offset).cast::<cmsghdr>()
        };
        let next_ptr = unsafe { CMSG_NXTHDR(dummy_msghdr, p_cmsghdr) }.cast::<MaybeUninit<cmsghdr>>();
        if next_ptr.is_null() {
            // Return early, since we have absolutely nothing to do if there's no next cmsghdr
            return None;
        }
        debug_assert!(
            next_ptr.cast::<u8>().cast_const() >= origin,
            "CMSG_NXTHDR gave a pointer outside of the buffer"
        );

        // No zero-fill here, `initialize_post_payload()` does that bit.

        unsafe {
            // SAFETY: we trust the implementation of CMSG_NXTHDR to do its thing correctly
            Some(&mut *next_ptr)
        }
    }
    /// Finds the last `cmsghdr` in a buffer that was previously initialized by some other routine, most likely the kernel (via `recv_ancillary`).
    ///
    /// # Safety
    /// `dummy_msghdr` is assumed to be filled in by a prior call of `make_msghdr(true)` on the same bytecrop of `self`.
    unsafe fn find_cmsghdr_offset_from_init(&mut self, dummy_msghdr: &msghdr) {
        let origin = self.buf.as_ptr_range().start;

        // today I will write an unsafe closure
        let tooffset = move |p: *mut MaybeUninit<cmsghdr>| unsafe {
            // SAFETY: always gonna be inside the buffer
            let offset = p.cast::<MaybeUninit<u8>>().offset_from(origin);
            debug_assert!(offset >= 0);
            offset as usize
        };
        let mut offset = unsafe {
            // SAFETY: same contract
            Self::first_cmsghdr(self.buf, dummy_msghdr).map(|r| tooffset(r as *mut MaybeUninit<cmsghdr>))
        };
        while let Some(voffset) = offset {
            let nxt = unsafe { Self::next_cmsghdr(self.buf, dummy_msghdr, voffset) };
            offset = nxt.map(|r| tooffset(r as *mut MaybeUninit<cmsghdr>));
            self.cmsghdr_offset = NonZeroUsize::new(voffset);
        }
    }
    /// Returns a reference to the next `cmsghdr`, depending on the value of `self.cmghdr_offset`: if it's `None`, uses `prepare_first_cmsghdr()`, and if it's `Some`, uses `CMSG_NXTHDR()`.
    ///
    /// # Safety
    /// `dummy_msghdr` is assumed to be filled in by a prior call of `make_msghdr(true)` on the same bytecrop of `self`.
    unsafe fn prepare_cmsghdr<'x>(&'x mut self, dummy_msghdr: &msghdr) -> Option<&'x mut MaybeUninit<cmsghdr>> {
        let origin = self.buf.as_ptr_range().start;

        let cmsghdr = unsafe {
            if self.cmsghdr_offset.is_none() && self.init_len != 0 {
                self.find_cmsghdr_offset_from_init(dummy_msghdr);
            }
            match self.cmsghdr_offset {
                None => Self::prepare_first_cmsghdr(self.buf, dummy_msghdr),
                Some(offset) => Self::next_cmsghdr(self.buf, dummy_msghdr, offset.get()),
            }?
        };

        let offset = unsafe {
            // SAFETY: the prepare methods return references within the slice, I promise!
            (cmsghdr as *mut MaybeUninit<cmsghdr>)
                .cast::<MaybeUninit<u8>>()
                .offset_from(origin)
        };
        debug_assert!(offset >= 0);
        let offset = offset as usize;

        self.cmsghdr_offset = Some(NonZeroUsize::new(offset).unwrap());
        Some(cmsghdr)
    }
    fn fill_cmsghdr(
        rhdr: &mut MaybeUninit<cmsghdr>,
        msg_len: c_uint,
        cmsg_level: c_int,
        cmsg_type: c_int,
    ) -> &mut cmsghdr {
        // Zero out all of the padding first, if any is present; will be elided by the compiler if unnecessary.
        rhdr.write(unsafe { zeroed() });

        let cmsg_len = to_cmsghdr_len(unsafe { CMSG_LEN(msg_len) }).unwrap();
        let chdr = cmsghdr {
            cmsg_len,
            cmsg_level,
            cmsg_type,
        };

        rhdr.write(chdr);
        unsafe {
            // SAFETY: we've filled out all the padding, if any, with zeroes, and the rest with the provided values
            rhdr.assume_init_mut()
        }
    }
    /// Fills bytes between the `cmsghdr` and the beginning of the payload range with zeroes and returns a reference to it.
    unsafe fn prepare_data_range(&mut self, hdr: *const cmsghdr, msg_len: usize) -> Option<&mut [MaybeUninit<u8>]> {
        assert_eq!(
            size_of::<c_char>(),
            size_of::<u8>(),
            "not supported on platforms where `char` is not a byte"
        );

        // Remove MaybeUninit from the pointer type, since pointers carry no initialization guarantee
        let one_past_end = self.buf.as_mut_ptr_range().end.cast::<u8>();

        // The safety check for the final cast is done at the top of the function.
        let data_start = unsafe { CMSG_DATA(hdr) }.cast::<u8>();
        let data_end = data_start.wrapping_add(msg_len);

        #[cfg(debug_assertions)]
        if data_start.is_null() {
            // We aren't actually required to do a null check, but let's do one here just in case.
            return None;
        }
        if data_end >= one_past_end {
            // The more important check here, the buffer overflow guard.
            return None;
        }

        let one_past_hdr = unsafe {
            // SAFETY: we checked for buffer overrun just above, so we know that the byte after the cmsghdr is inside
            // the allocated object (besides, .offset() even allows you to go one byte past).
            hdr.cast::<u8>().cast_mut().offset(1)
        };
        if data_start > one_past_hdr {
            unsafe {
                // SAFETY: we just ensured that both are within the same allocated object.
                let fill_len = data_start.offset_from(one_past_hdr);
                debug_assert!(fill_len > 0);
                // SAFETY: see prepare_first_hdr().
                ptr::write_bytes(one_past_hdr, 0, fill_len as usize);
            }
        }

        Some(unsafe {
            // SAFETY: we just checked that the message fits within the buffer
            slice::from_raw_parts_mut(data_start.cast::<MaybeUninit<u8>>(), msg_len)
        })
    }
    /// Initializes the bytes that are either padding between the current control message and the next or dead space at the end of the buffer, returning how much `self`'s init cursor needs to be advanced by.
    ///
    /// # Safety
    /// - The dummy `msghdr` must be the product of `make_msghdr(true)` called on the exact same bytecrop of `self` (same base address, same length).
    /// - `cmsghdr` must be non-null, must point somewhere within `self`'s buffer and must be, well, the `cmsghdr` the payload of which has just been filled in.
    /// - `one_past_end_of_payload` must be a pointer to the byte directly past the end of the data payload, the base address of which is the output of `CMSG_DATA` and the length of which is the output of `CMSG_LEN`.
    unsafe fn initialize_post_payload(
        &mut self,
        dummy_msghdr: &msghdr,
        cmsghdr: *const cmsghdr,
        one_past_end_of_payload: *mut u8,
    ) -> usize {
        let origin_of_uninit = unsafe {
            // SAFETY: the init cursor isn't borked... is it?
            self.buf.as_ptr_range().start.cast::<u8>().add(self.init_len)
        };
        let next = unsafe { CMSG_NXTHDR(dummy_msghdr, cmsghdr) }.cast::<u8>();

        let (fill_len, init_cur_offset) = if !next.is_null() {
            let fill_len = unsafe {
                // SAFETY: we made sure that both are non-null and within the same allocated object
                next.offset_from(one_past_end_of_payload)
            };
            let init_cur_offset = unsafe {
                // SAFETY: same here.
                next.offset_from(origin_of_uninit)
            };
            (fill_len, init_cur_offset)
        } else {
            let one_past_end = self.buf.as_mut_ptr_range().end.cast::<u8>();
            let fill_len = unsafe {
                // SAFETY: we, again, made sure that both pointers are non-null and are within the same allocated
                // object, but this time one of them is actually one byte past, which is fine because the .offset()
                // family allows that.
                one_past_end.offset_from(one_past_end_of_payload.cast_const())
            };
            let init_cur_offset = unsafe {
                // SAFETY: same here.
                one_past_end.offset_from(origin_of_uninit)
            };
            (fill_len, init_cur_offset)
        };

        debug_assert!(fill_len >= 0);
        unsafe {
            // SAFETY: pointer validity is ensured, alignment requirements are irrelevant for bytes.
            ptr::write_bytes(one_past_end_of_payload, 0, fill_len as usize);
        };

        debug_assert!(init_cur_offset >= 0);
        init_cur_offset as usize
    }

    /// Converts the given message object to a [`Cmsg`] and adds it to the buffer, advances the initialization cursor of `self` such that the next message, if one is added, will appear after it, and returns how much the cursor was advanced by (i.e. how many more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// If there isn't enough space, 0 is returned.
    pub fn add_message(&mut self, msg: &impl ToCmsg) -> usize {
        let mut ret = 0;
        msg.add_to_buffer(|cmsg| ret = self.add_raw_message(cmsg));
        ret
    }
    /// Adds the specified control message to the buffer, advances the initialization cursor of `self` such that the next message, if one is added, will appear after it, and returns how much the cursor was advanced by (i.e. how many more contiguous bytes in the beginning of `self`'s buffer are now well-initialized).
    ///
    /// If there isn't enough space, 0 is returned.
    pub fn add_raw_message(&mut self, cmsg: Cmsg<'_>) -> usize {
        let uninit_buf_len = self.buf.len() - self.init_len;
        if uninit_buf_len < size_of::<cmsghdr>() {
            return 0;
        }

        let msg_len: c_uint = cmsg
            .data()
            .len()
            .try_into()
            .expect("could not convert message length to `unsigned int`");

        let dummy_msghdr = self.make_msghdr(true);
        let cmsghdr = match unsafe { self.prepare_cmsghdr(&dummy_msghdr) } {
            Some(h) => h,
            None => return 0,
        };
        let cmsghdr: *const _ = &*Self::fill_cmsghdr(cmsghdr, msg_len, cmsg.cmsg_level(), cmsg.cmsg_type());

        let data = match unsafe {
            // SAFETY: the only contract here is that the cmsghdr pointer is valid (lifetime bypass)
            self.prepare_data_range(cmsghdr, cmsg.data().len())
        } {
            Some(d) => d,
            None => return 0,
        };
        let msg_uninit = unsafe {
            // SAFETY: this is a relaxation of the init guarantee
            transmute::<&[u8], &[MaybeUninit<u8>]>(cmsg.data)
        };
        data.copy_from_slice(msg_uninit);

        let one_past_end_of_payload = data.as_mut_ptr_range().end.cast::<u8>();
        let init_cur_incr = unsafe {
            // SAFETY: dummy_msghdr is the correct thing, cmsghdr is as well, one_past_end_of_payload being correct
            // is the public API of slices
            self.initialize_post_payload(&dummy_msghdr, cmsghdr, one_past_end_of_payload)
        };

        self.init_len += init_cur_incr;
        init_cur_incr
    }
}
impl<'a> TryFrom<&'a mut [MaybeUninit<u8>]> for CmsgMut<'a> {
    type Error = BufferTooBig<&'a mut [MaybeUninit<u8>], MaybeUninit<u8>>;
    #[inline]
    fn try_from(buf: &'a mut [MaybeUninit<u8>]) -> Result<Self, Self::Error> {
        if buf.len() > isize::MAX as usize {
            return Err(BufferTooBig(buf));
        }
        Ok(Self {
            buf,
            init_len: 0,
            cmsghdr_offset: None,
        })
    }
}
