use crate::os::unix::{
    udsocket::cmsg::{CmsgMut, CmsgRef},
    unixprelude::*,
};
use cfg_if::cfg_if;
use libc::{iovec, msghdr};
use std::{
    ffi::{CStr, CString},
    hint::unreachable_unchecked,
    io::{self, IoSlice, IoSliceMut},
    ptr,
};
use to_method::To;

pub const DUMMY_MSGHDR: msghdr = msghdr {
    msg_name: ptr::null_mut(),
    msg_namelen: 0,
    msg_iov: ptr::null_mut(),
    msg_iovlen: 0,
    msg_control: ptr::null_mut(),
    msg_controllen: 0,
    msg_flags: 0,
};

#[allow(dead_code)]
mod tname {
    pub static SOCKLEN_T: &str = "`socklen_t`";
    pub static SIZE_T: &str = "`size_t`";
    pub static C_INT: &str = "`c_int`";
}

cfg_if! {
    if #[cfg(uds_msghdr_iovlen_c_int)] {
        pub type MsghdrIovlen = c_int;
        static MSGHDR_IOVLEN_NAME: &str = tname::C_INT;
    } else if #[cfg(uds_msghdr_iovlen_size_t)] {
        pub type MsghdrIovlen = size_t;
        static MSGHDR_IOVLEN_NAME: &str = tname::SIZE_T;
    }
}
cfg_if! {
    if #[cfg(uds_msghdr_controllen_socklen_t)] {
        pub type MsghdrControllen = libc::socklen_t;
        static MSGHDR_CONTROLLEN_NAME: &str = tname::SOCKLEN_T;
} else if #[cfg(uds_msghdr_controllen_size_t)] {
        pub type MsghdrControllen = size_t;
        static MSGHDR_CONTROLLEN_NAME: &str = tname::SIZE_T;
    }
}

pub fn to_msghdr_iovlen(iovlen: usize) -> io::Result<MsghdrIovlen> {
    iovlen.try_to::<MsghdrIovlen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("number of scatter-gather buffers overflowed {MSGHDR_IOVLEN_NAME}"),
        )
    })
}
pub fn to_msghdr_controllen(controllen: usize) -> io::Result<MsghdrControllen> {
    controllen.try_to::<MsghdrControllen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ancillary data buffer length overflowed {MSGHDR_CONTROLLEN_NAME}"),
        )
    })
}
pub fn empty_cstring() -> CString {
    unsafe {
        // SAFETY: the value returned by Vec::new() is always empty, thus it
        // adheres to the contract of CString::new().
        CString::new(Vec::new()).unwrap_or_else(|_| unreachable_unchecked())
    }
}
pub fn empty_cstr() -> &'static CStr {
    unsafe {
        // SAFETY: a single nul terminator is a valid CStr
        CStr::from_bytes_with_nul_unchecked(&[0])
    }
}

pub fn make_msghdr_r(bufs: &mut [IoSliceMut<'_>], abuf: &mut CmsgMut<'_>) -> io::Result<msghdr> {
    let mut hdr = DUMMY_MSGHDR;
    _fill_out_msghdr(
        &mut hdr,
        bufs.as_mut_ptr().cast::<iovec>(),
        to_msghdr_iovlen(bufs.len())?,
    );
    abuf.fill_msghdr(&mut hdr, true)?;
    Ok(hdr)
}
pub fn make_msghdr_w(bufs: &[IoSlice<'_>], abuf: CmsgRef<'_>) -> io::Result<msghdr> {
    let mut hdr = DUMMY_MSGHDR;
    _fill_out_msghdr(
        &mut hdr,
        bufs.as_ptr().cast_mut().cast::<iovec>(),
        to_msghdr_iovlen(bufs.len())?,
    );
    abuf.fill_msghdr(&mut hdr)?;
    Ok(hdr)
}
fn _fill_out_msghdr(hdr: &mut msghdr, iov: *mut iovec, iovlen: MsghdrIovlen) {
    hdr.msg_iov = iov;
    hdr.msg_iovlen = iovlen;
}

pub fn eunreachable<T, U>(_e: T) -> U {
    unreachable!()
}
