use super::imports::*;
use cfg_if::cfg_if;
use std::{
    ffi::{CStr, CString},
    hint::unreachable_unchecked,
    io::{self, IoSlice, IoSliceMut},
    mem::zeroed,
};
use to_method::To;

#[cfg(unix)]
#[allow(dead_code)]
mod tname {
    pub static SOCKLEN_T: &str = "`socklen_t`";
    pub static SIZE_T: &str = "`size_t`";
    pub static C_INT: &str = "`c_int`";
}

#[cfg(unix)]
cfg_if! {
    if #[cfg(uds_msghdr_iovlen_c_int)] {
        pub type MsghdrIovlen = c_int;
        static MSGHDR_IOVLEN_NAME: &str = tname::C_INT;
    } else if #[cfg(uds_msghdr_iovlen_size_t)] {
        pub type MsghdrIovlen = size_t;
        static MSGHDR_IOVLEN_NAME: &str = tname::SIZE_T;
    }
}
#[cfg(unix)]
cfg_if! {
    if #[cfg(uds_msghdr_controllen_socklen_t)] {
        pub type MsghdrControllen = socklen_t;
        static MSGHDR_CONTROLLEN_NAME: &str = tname::SOCKLEN_T;
} else if #[cfg(uds_msghdr_controllen_size_t)] {
        pub type MsghdrControllen = size_t;
        static MSGHDR_CONTROLLEN_NAME: &str = tname::SIZE_T;
    }
}

#[cfg(unix)]
pub fn to_msghdr_iovlen(iovlen: usize) -> io::Result<MsghdrIovlen> {
    iovlen.try_to::<MsghdrIovlen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "number of scatter-gather buffers overflowed {}",
                MSGHDR_IOVLEN_NAME,
            ),
        )
    })
}
#[cfg(unix)]
pub fn to_msghdr_controllen(controllen: usize) -> io::Result<MsghdrControllen> {
    controllen.try_to::<MsghdrControllen>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ancillary data buffer length overflowed {}",
                MSGHDR_CONTROLLEN_NAME,
            ),
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

pub fn fill_out_msghdr_r(
    hdr: &mut msghdr,
    iov: &mut [IoSliceMut<'_>],
    anc: &mut [u8],
) -> io::Result<()> {
    _fill_out_msghdr(
        hdr,
        iov.as_ptr() as *mut _,
        to_msghdr_iovlen(iov.len())?,
        anc.as_mut_ptr(),
        to_msghdr_controllen(anc.len())?,
    )
}
pub fn fill_out_msghdr_w(hdr: &mut msghdr, iov: &[IoSlice<'_>], anc: &[u8]) -> io::Result<()> {
    _fill_out_msghdr(
        hdr,
        iov.as_ptr() as *mut _,
        to_msghdr_iovlen(iov.len())?,
        anc.as_ptr() as *mut _,
        to_msghdr_controllen(anc.len())?,
    )
}
#[cfg(unix)]
fn _fill_out_msghdr(
    hdr: &mut msghdr,
    iov: *mut iovec,
    iovlen: MsghdrIovlen,
    control: *mut u8,
    controllen: MsghdrControllen,
) -> io::Result<()> {
    hdr.msg_iov = iov;
    hdr.msg_iovlen = iovlen;
    hdr.msg_control = control as *mut _;
    hdr.msg_controllen = controllen;
    Ok(())
}
pub fn mk_msghdr_r(iov: &mut [IoSliceMut<'_>], anc: &mut [u8]) -> io::Result<msghdr> {
    let mut hdr = unsafe {
        // SAFETY: msghdr is plain old data, i.e. an all-zero pattern is allowed
        zeroed()
    };
    fill_out_msghdr_r(&mut hdr, iov, anc)?;
    Ok(hdr)
}
pub fn mk_msghdr_w(iov: &[IoSlice<'_>], anc: &[u8]) -> io::Result<msghdr> {
    let mut hdr = unsafe {
        // SAFETY: msghdr is plain old data, i.e. an all-zero pattern is allowed
        zeroed()
    };
    fill_out_msghdr_w(&mut hdr, iov, anc)?;
    Ok(hdr)
}
pub fn check_ancillary_unsound() -> io::Result<()> {
    if cfg!(uds_ancillary_unsound) {
        let error_kind = {
            #[cfg(io_error_kind_unsupported_stable)]
            {
                io::ErrorKind::Unsupported
            }
            #[cfg(not(io_error_kind_unsupported_stable))]
            {
                io::ErrorKind::Other
            }
        };
        Err(io::Error::new(
            error_kind,
            "\
ancillary data has been disabled for non-x86 ISAs in a hotfix because it \
doesn't account for alignment",
        ))
    } else {
        Ok(())
    }
}

pub fn eunreachable<T, U>(_e: T) -> U {
    unreachable!()
}
