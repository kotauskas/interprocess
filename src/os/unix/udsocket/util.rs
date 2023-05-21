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

// TODO add type of cmsg_len

cfg_if! {
    if #[cfg(uds_msghdr_iovlen_c_int)] {
        pub type MsghdrIovlen = c_int;
        macro_rules! msghdr_iovlen_name {
            () => {"c_int"}
        }
    } else if #[cfg(uds_msghdr_iovlen_size_t)] {
        pub type MsghdrIovlen = size_t;
        macro_rules! msghdr_iovlen_name {
            () => {"size_t"}
        }
    }
}
cfg_if! {
    if #[cfg(uds_msghdr_controllen_socklen_t)] {
        pub type MsghdrControllen = libc::socklen_t;
        macro_rules! msghdr_controllen_name {
            () => {"socklen_t"}
        }
} else if #[cfg(uds_msghdr_controllen_size_t)] {
        pub type MsghdrControllen = size_t;
        macro_rules! msghdr_controllen_name {
            () => {"size_t"}
        }
    }
}
cfg_if! {
    if #[cfg(uds_cmsghdr_len_socklen_t)] {
        pub type CmsghdrLen = libc::socklen_t;
        macro_rules! cmsghdr_len_name {
            () => {"socklen_t"}
        }
    }
    else if #[cfg(uds_cmsghdr_len_size_t)] {
        pub type CmsghdrLen = libc::size_t;
        macro_rules! cmsghdr_len_name {
            () => {"size_t"}
        }
    }
}

pub fn to_msghdr_iovlen(iovlen: usize) -> io::Result<MsghdrIovlen> {
    iovlen.try_to::<MsghdrIovlen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            concat!("number of scatter-gather buffers overflowed ", msghdr_iovlen_name!()),
        )
    })
}
pub fn to_msghdr_controllen(controllen: usize) -> io::Result<MsghdrControllen> {
    controllen.try_to::<MsghdrControllen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            concat!("control message buffer length overflowed ", msghdr_controllen_name!()),
        )
    })
}
pub fn to_cmsghdr_len<T: TryInto<CmsghdrLen>>(cmsg_len: T) -> io::Result<CmsghdrLen> {
    cmsg_len.try_to::<CmsghdrLen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            concat!("control message length overflowed ", cmsghdr_len_name!()),
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
