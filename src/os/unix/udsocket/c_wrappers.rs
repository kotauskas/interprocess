use crate::os::unix::{unixprelude::*, FdOps};
use libc::{
    msghdr, sockaddr, sockaddr_un, socklen_t, AF_UNIX, F_GETFL, F_SETFL, O_NONBLOCK, SHUT_RD, SHUT_RDWR, SHUT_WR,
};
use std::{
    ffi::c_void,
    io,
    mem::{size_of, size_of_val},
    net::Shutdown,
    ptr,
};
use to_method::To;

#[cfg_attr(target_os = "linux", allow(unused))]
pub(super) use crate::os::unix::c_wrappers::*;

pub(super) fn create_uds(ty: c_int, nonblocking: bool) -> io::Result<FdOps> {
    #[allow(unused_mut, clippy::let_and_return)]
    let ty = {
        let mut ty = ty;
        #[cfg(target_os = "linux")]
        {
            ty |= libc::SOCK_CLOEXEC;
            if nonblocking {
                ty |= libc::SOCK_NONBLOCK;
            }
        }
        ty
    };
    let fd = create_uds_raw(ty)?;
    #[cfg(not(target_os = "linux"))]
    {
        set_nonblocking(fd.0.as_fd(), nonblocking)?;
        set_cloexec(fd.0.as_fd(), true)?;
    }
    Ok(fd)
}
fn create_uds_raw(ty: c_int) -> io::Result<FdOps> {
    let (success, fd) = unsafe {
        let result = libc::socket(AF_UNIX, ty, 0);
        (result != -1, result)
    };
    if success {
        let fdops = unsafe {
            // SAFETY: we just created this descriptor
            FdOps::from_raw_fd(fd)
        };
        Ok(fdops)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Reads stream data and ancillary data from the given socket. Pointers are supplied directly via the `msghdr`.
///
/// # Safety
/// Pointers in `hdr` must not dangle, and ancillary data must be correct.
#[allow(unused_mut)]
pub(super) unsafe fn recvmsg(fd: BorrowedFd<'_>, hdr: &mut msghdr, mut flags: c_int) -> io::Result<usize> {
    #[cfg(target_os = "linux")]
    {
        flags |= libc::MSG_CMSG_CLOEXEC;
    }

    let (success, bytes_read) = unsafe {
        let result = libc::recvmsg(fd.as_raw_fd(), hdr, flags);
        (result != -1, result as usize)
    };

    ok_or_ret_errno!(success => bytes_read)
}
/// Writes stream data and ancillary data from the given socket. Pointers are supplied directly via the `msghdr`.
///
/// # Safety
/// Pointers in `hdr` must not dangle, and ancillary data must be correct.
pub(super) unsafe fn sendmsg(fd: BorrowedFd<'_>, hdr: &msghdr, flags: c_int) -> io::Result<usize> {
    let (success, bytes_written) = unsafe {
        let result = libc::sendmsg(fd.as_raw_fd(), hdr, flags);
        (result != -1, result as usize)
    };
    ok_or_ret_errno!(success => bytes_written)
}

/// Binds the specified Ud-socket file descriptor to the given address.
///
/// # Safety
/// `addr` must be properly null-terminated.
pub(super) unsafe fn bind(fd: BorrowedFd<'_>, addr: &sockaddr_un) -> io::Result<()> {
    let success = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            // Double cast because you cannot cast a reference to a pointer of arbitrary type
            // but you can cast any narrow pointer to any other narrow pointer
            addr as *const _ as *const sockaddr,
            size_of::<sockaddr_un>() as u32,
        ) != -1
    };
    ok_or_ret_errno!(success => ())
}

/// Connects the specified Ud-socket file descriptor to the given address.
///
/// # Safety
/// `addr` must be properly null-terminated.
pub(super) unsafe fn connect(fd: BorrowedFd<'_>, addr: &sockaddr_un) -> io::Result<()> {
    let success = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (addr as *const sockaddr_un).cast(),
            size_of::<sockaddr_un>() as _,
        ) != -1
    };
    ok_or_ret_errno!(success => ())
}

pub(super) fn listen(fd: BorrowedFd<'_>, backlog: c_int) -> io::Result<()> {
    let success = unsafe { libc::listen(fd.as_raw_fd(), backlog) != -1 };
    ok_or_ret_errno!(success => ())
}

#[allow(dead_code)]
pub(super) unsafe fn set_socket_option<T>(fd: BorrowedFd<'_>, level: c_int, option: c_int, val: &T) -> io::Result<()> {
    let ptr = <*const _>::cast::<c_void>(val);
    let len = socklen_t::try_from(size_of_val(val)).unwrap();
    let success = unsafe { libc::setsockopt(fd.as_raw_fd(), level, option, ptr, len) != -1 };
    ok_or_ret_errno!(success => ())
}

pub(super) fn get_socket_option<T>(fd: BorrowedFd<'_>, level: c_int, option: c_int, buf: &mut T) -> io::Result<usize> {
    let ptr = <*mut _>::cast::<c_void>(buf);
    let mut len = socklen_t::try_from(size_of_val(buf)).unwrap();
    let success = unsafe { libc::getsockopt(fd.as_raw_fd(), level, option, ptr, &mut len) != -1 };
    ok_or_ret_errno!(success => len.try_into().unwrap())
}

#[cfg(uds_sockcred)]
fn set_local_creds(fd: BorrowedFd<'_>, creds: bool) -> io::Result<()> {
    unsafe { set_socket_option(fd, libc::SOL_SOCKET, libc::LOCAL_CREDS, &creds.to::<c_int>()) }
}
#[cfg(uds_sockcred)]
fn set_local_creds_persistent(fd: BorrowedFd<'_>, creds: bool) -> io::Result<()> {
    unsafe { set_socket_option(fd, libc::SOL_SOCKET, libc::LOCAL_CREDS_PERSISTENT, &creds.to::<c_int>()) }
}
#[cfg(any(uds_ucred, uds_sockcred))]
pub(super) fn set_persistent_ancillary_cred(fd: BorrowedFd<'_>, val: bool) -> io::Result<()> {
    #[cfg(uds_ucred)]
    {
        unsafe { set_socket_option(fd, libc::SOL_SOCKET, libc::SO_PASSCRED, &val.to::<c_int>()) }
    }
    #[cfg(uds_sockcred)]
    {
        // TODO SCM_CREDS2
        set_local_creds_persistent(fd, val)
    }
}
#[cfg(uds_sockcred)]
pub(super) fn set_oneshot_ancillary_cred(fd: BorrowedFd<'_>, val: bool) -> io::Result<()> {
    if val {
        // LOCAL_CREDS and LOCAL_CREDS_PERSISTENT are mutually exclusive
        let _ = set_local_creds_persistent(fd, false);
    }
    set_local_creds(fd, val)
}
#[cfg(uds_peerucred)]
pub(super) fn get_peer_ucred(fd: BorrowedFd<'_>) -> io::Result<libc::ucred> {
    use libc::ucred;
    let mut cred = ucred { pid: 0, uid: 0, gid: 0 };
    get_socket_option(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred)?;
    Ok(cred)
}
fn get_status_flags(fd: BorrowedFd<'_>) -> io::Result<c_int> {
    let (flags, success) = unsafe {
        // SAFETY: nothing too unsafe about this function. One thing to note is that we're passing
        // it a null pointer, which is, for some reason, required yet ignored for F_GETFL.
        let result = libc::fcntl(fd.as_raw_fd(), F_GETFL, ptr::null::<c_void>());
        (result, result != -1)
    };
    ok_or_ret_errno!(success => flags)
}
fn set_status_flags(fd: BorrowedFd<'_>, new_flags: c_int) -> io::Result<()> {
    let success = unsafe {
        // SAFETY: new_flags is a c_int, as documented in the manpage.
        libc::fcntl(fd.as_raw_fd(), F_SETFL, new_flags)
    } != -1;
    ok_or_ret_errno!(success => ())
}
pub(super) fn set_nonblocking(fd: BorrowedFd<'_>, nonblocking: bool) -> io::Result<()> {
    let old_flags = get_status_flags(fd)?;
    let new_flags = if nonblocking {
        old_flags | O_NONBLOCK
    } else {
        // Inverting the O_NONBLOCK value sets all the bits in the flag set to 1 except for the
        // nonblocking flag, which clears the flag when ANDed.
        old_flags & !O_NONBLOCK
    };
    set_status_flags(fd, new_flags)
}
pub(super) fn get_nonblocking(fd: BorrowedFd<'_>) -> io::Result<bool> {
    let flags = get_status_flags(fd)?;
    Ok(flags & O_NONBLOCK != 0)
}
pub(super) fn shutdown(fd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
    let how = match how {
        Shutdown::Read => SHUT_RD,
        Shutdown::Write => SHUT_WR,
        Shutdown::Both => SHUT_RDWR,
    };
    let success = unsafe { libc::shutdown(fd.as_raw_fd(), how) != -1 };
    ok_or_ret_errno!(success => ())
}
