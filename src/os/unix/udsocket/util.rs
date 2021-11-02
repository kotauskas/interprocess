use super::imports::*;
use cfg_if::cfg_if;
#[cfg(uds_supported)]
use std::net::Shutdown;
use std::{
    ffi::{c_void, CStr, CString},
    hint::unreachable_unchecked,
    io::{self, IoSlice, IoSliceMut},
    mem::{size_of, size_of_val, zeroed},
    ptr::null,
};
use to_method::To;

//pub type MsghdrNamelen = socklen_t;

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
        pub type MsghdrControllen = socklen_t;
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
            format!(
                "number of scatter-gather buffers overflowed {}",
                MSGHDR_IOVLEN_NAME,
            ),
        )
    })
}
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

#[allow(unused_variables)]
pub unsafe fn enable_passcred(socket: i32) -> io::Result<()> {
    #[cfg(uds_scm_credentials)]
    {
        let passcred: c_int = 1;
        let success = unsafe {
            libc::setsockopt(
                socket,
                SOL_SOCKET,
                SO_PASSCRED,
                &passcred as *const _ as *const _,
                size_of_val(&passcred).try_to::<u32>().unwrap(),
            )
        } != -1;
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(uds_scm_credentials))]
    {
        Ok(())
    }
}
#[cfg(uds_peercred)]
pub unsafe fn get_peer_ucred(socket: i32) -> io::Result<ucred> {
    let mut cred: ucred = unsafe {
        // SAFETY: it's safe for the ucred structure to be zero-initialized, since
        // it only contains integers
        zeroed()
    };
    let mut cred_len = size_of::<ucred>() as socklen_t;
    let success = unsafe {
        libc::getsockopt(
            socket,
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut cred_len as *mut _,
        )
    } != -1;
    if success {
        Ok(cred)
    } else {
        // This used to delegate error handling to the outer function, but I changed it to do it
        // here because the function had thread-local state associated with it which persisted
        // past the moment it returned — it's part of the function's signature, in some way,
        // that errno contains the error result after the function is called, meaning that
        // leaving usable data in global variables is part of its API, and that's a bad pratice.
        Err(io::Error::last_os_error())
    }
}
pub unsafe fn raw_set_nonblocking(socket: i32, nonblocking: bool) -> io::Result<()> {
    let (old_flags, success) = unsafe {
        // SAFETY: nothing too unsafe about this function. One thing to note is that we're passing
        // it a null pointer, which is, for some reason, required yet ignored for F_GETFL.
        let result = libc::fcntl(socket, F_GETFL, null::<c_void>());
        (result, result != -1)
    };
    if !success {
        return Err(io::Error::last_os_error());
    }
    let new_flags = if nonblocking {
        old_flags | O_NONBLOCK
    } else {
        // Inverting the O_NONBLOCK value sets all the bits in the flag set to 1 except for the
        // nonblocking flag, which clears the flag when ANDed.
        old_flags & !O_NONBLOCK
    };
    let success = unsafe {
        // SAFETY: new_flags is a c_int, as documented in the manpage.
        libc::fcntl(socket, F_SETFL, new_flags)
    } != -1;
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
pub unsafe fn raw_get_nonblocking(socket: i32) -> io::Result<bool> {
    let flags = unsafe {
        // SAFETY: exactly the same as above.
        libc::fcntl(socket, F_GETFL, null::<c_void>())
    };
    if flags != -1 {
        Ok(flags & O_NONBLOCK != 0)
    } else {
        // Again, querying errno was previously left to the outer function but is now done here.
        Err(io::Error::last_os_error())
    }
}
#[cfg(uds_supported)]
pub unsafe fn raw_shutdown(socket: i32, how: Shutdown) -> io::Result<()> {
    let how = match how {
        Shutdown::Read => SHUT_RD,
        Shutdown::Write => SHUT_WR,
        Shutdown::Both => SHUT_RDWR,
    };
    let success = unsafe { libc::shutdown(socket, how) } != -1;
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
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
