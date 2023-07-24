//! Insertion of ancillary messages into mutable buffers.

use super::{super::Cmsg, *};
use crate::{os::unix::udsocket::util::to_msghdr_controllen, weaken_buf_init};
use libc::{cmsghdr, msghdr};
use std::{
    ffi::c_void,
    mem::{align_of, size_of, zeroed},
};

type MUu8 = MaybeUninit<u8>;
const ZEROFILL: MUu8 = MUu8::new(0);

fn dummy_msghdr(buf: &[MUu8]) -> msghdr {
    let mut hdr = unsafe { zeroed::<msghdr>() };
    hdr.msg_control = buf.as_ptr().cast::<c_void>().cast_mut();
    hdr.msg_controllen = to_msghdr_controllen(buf.len()).expect("failed to create dummy msghdr");
    hdr
}

/// Computes an index to the first byte in the buffer in which a `cmsghdr` would be well-aligned.
///
/// The returned location is guaranteed to be able to fit a `cmsghdr`.
// Used in read.rs
pub(super) fn align_first(buf: &[MUu8]) -> Option<usize> {
    const ALIGN: usize = align_of::<cmsghdr>();
    const MASK: usize = ALIGN - 1;

    // The potentially misaligned address
    let base = buf.as_ptr() as usize;
    // Adding the mask pushes any misaligned address over the edge, but puts a well-aligned one
    // just a single byte short of going forward by the alignment's worth of offset
    let nudged = base + MASK;
    // Masking the misalignment out puts us at an aligned location
    let aligned = nudged & !MASK;
    // The amount by which the start must be moved forward to become aligned
    let fwd_align = aligned - base;

    let mut hdr = dummy_msghdr(buf);
    hdr.msg_control = hdr.msg_control.wrapping_add(fwd_align);
    // The cast here is fine because no one in their right mind expects the alignment adjustment to
    // exceed the sort of integer type used for controllen
    hdr.msg_controllen = hdr.msg_controllen.saturating_sub(fwd_align as _);

    let base = unsafe { libc::CMSG_FIRSTHDR(&hdr) }.cast::<MUu8>();
    if base.is_null() {
        return None;
    }

    let base_idx = unsafe {
        // SAFETY: CMSG_FIRSTHDR never returns a pointer outside the buffer if the return value is non-null
        base.offset_from(buf.as_ptr())
    };
    debug_assert!(base_idx >= 0);
    Some(base_idx as usize)
}

/// Wraps `CMSG_NXTHDR` in offset-addressing form. The current `cmsghdr` from which the next one is to be found is
/// assumed to be at the beginning of the buffer.
fn locate_next_cmsghdr_idx(buf: &[MUu8]) -> Option<usize> {
    let cur = buf.as_ptr().cast::<cmsghdr>();

    let hdr = dummy_msghdr(buf);
    let base = unsafe {
        // SAFETY: all passed pointers are derived from references
        libc::CMSG_NXTHDR(&hdr, cur)
    };
    if base.is_null() {
        return None;
    }
    let base_idx = unsafe {
        // SAFETY: CMSG_NXTHDR never returns a pointer outside the buffer if the return value is non-null
        base.offset_from(cur)
    };
    debug_assert!(base_idx >= 0);
    Some(base_idx as usize)
}

/// Adds the control message `cmsg` to `buf` and returns the amount by which it was well-initialized. Initialization
/// cursor is moved accordingly.
pub(super) fn add_raw_message(buf: &mut (impl CmsgMut + ?Sized), cmsg: Cmsg<'_>) -> usize {
    // This will be the return value.
    let mut ret = 0;

    let _ = buf.reserve(cmsg.space_occupied());

    let Some(fwd_align) = align_first(buf.uninit_part()) else {
        return 0;
    };
    buf.uninit_part()[..fwd_align].fill(ZEROFILL);
    unsafe {
        // SAFETY: we just filled that much with zeroes
        buf.add_len(fwd_align);
    }
    // Note that `uninit_part()` gets subsliced by `add_len()`.
    ret += fwd_align;

    if buf.uninit_part().len() < cmsg.space_occupied() {
        return 0;
    }

    // From this point on, for panic safety's sake, this variable will be used to keep track of the initialization
    // cursor increment by having increments next to the code that justifies them. It gets added to ret at the end
    // of the function.
    let mut valid_incr = 0;

    let data_base = {
        let m_chdr = unsafe {
            // SAFETY: By this point, `uninit_part()` is well-aligned and has its beginning pointer at
            // the location where a new cmsghdr ought to be.
            &mut *buf.uninit_part().as_mut_ptr().cast::<MaybeUninit<cmsghdr>>()
        };

        m_chdr.write(cmsghdr {
            cmsg_len: cmsg.cmsg_len() as _, // It does a check
            cmsg_level: cmsg.cmsg_level(),
            cmsg_type: cmsg.cmsg_type(),
        });
        valid_incr += size_of::<cmsghdr>();
        // Note that we don't advance the init cursor here just yet because that cmsg_len there at this moment lies
        // about the control message contents actually being in the buffer in a well-initialized and valid form.

        unsafe {
            // SAFETY: the macro performs a simple pointer addition; a quick peek under the hood reveals that it is
            // simply an .offset(1) call followed by a cast to a byte pointer. (This is most evident in the Rust libc
            // sources for Linux; the FreeBSD side of things, for example, does something a little more confusing but
            // functionally identical.)
            libc::CMSG_DATA((m_chdr as *mut MaybeUninit<cmsghdr>).cast::<cmsghdr>().cast_const())
        }
        .cast::<MUu8>()
    };
    let data_base_offset = unsafe {
        // SAFETY: the CMSG_SPACE check above ensures that data_base is within the buffer
        data_base.cast_const().offset_from(buf.uninit_part().as_ptr())
    };
    debug_assert!(data_base_offset >= 0);
    let data_base_offset = data_base_offset as usize;

    // The current cmsghdr is at offset 0, so one byte past the end is at this offset.
    let end_of_cmsgdhr = size_of::<cmsghdr>();

    // The spacer between the end of the current cmsghdr and the start of the control message body. This will usually
    // have a size of zero, and a good codegen might just inline enough things to optimize this bit out of existence.
    let pre_data_spacer = &mut buf.uninit_part()[end_of_cmsgdhr..data_base_offset];
    pre_data_spacer.fill(ZEROFILL);
    valid_incr += pre_data_spacer.len();

    let end_of_data_range = data_base_offset + cmsg.data().len();

    let data_range = &mut buf.uninit_part()[data_base_offset..end_of_data_range];
    data_range.copy_from_slice(weaken_buf_init(cmsg.data()));
    valid_incr += data_range.len();

    // Get an offset to the end of the buffer if another control message wouldn't fit.
    let next_cmsghdr_base_offset = locate_next_cmsghdr_idx(buf.uninit_part()).unwrap_or_else(|| buf.capacity());

    // The spacer between the end of the control message body and the next cmsghdr.
    let post_data_spacer = &mut buf.uninit_part()[end_of_data_range..next_cmsghdr_base_offset];
    post_data_spacer.fill(ZEROFILL);
    valid_incr += post_data_spacer.len();

    unsafe {
        // SAFETY: if you look at every increment of valid_incr closely, you will see that every single one of those
        // is associated with an initializing write to the buffer.
        buf.add_len(valid_incr);
    }
    ret += valid_incr;

    ret
}
