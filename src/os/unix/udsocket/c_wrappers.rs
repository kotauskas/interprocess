use crate::os::unix::{unixprelude::*, FdOps};
use libc::{sockaddr, sockaddr_un, AF_UNIX, F_GETFL, F_SETFL, O_NONBLOCK, SHUT_RD, SHUT_RDWR, SHUT_WR};
use std::{ffi::c_void, io, mem::size_of, net::Shutdown, ptr};

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

pub(super) fn set_passcred(fd: BorrowedFd<'_>, passcred: bool) -> io::Result<()> {
    #[cfg(uds_scm_credentials)]
    {
        use libc::{SOL_SOCKET, SO_PASSCRED};
        use std::mem::size_of_val;

        let passcred = passcred as c_int;
        let success = unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                SOL_SOCKET,
                SO_PASSCRED,
                &passcred as *const _ as *const _,
                size_of_val(&passcred) as u32,
            ) != -1
        };
        ok_or_ret_errno!(success => ())
    }
    #[cfg(not(uds_scm_credentials))]
    {
        let _ = (fd, passcred);
        Ok(())
    }
}
#[cfg(uds_peerucred)]
pub(super) fn get_peer_ucred(fd: BorrowedFd<'_>) -> io::Result<libc::ucred> {
    use libc::{socklen_t, ucred, SOL_SOCKET, SO_PEERCRED};
    use std::mem::zeroed;

    let mut cred = unsafe {
        // SAFETY: it's safe for the ucred structure to be zero-initialized, since
        // it only contains integers
        zeroed::<ucred>()
    };
    let mut cred_len = size_of::<ucred>() as socklen_t;
    let success = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut cred_len as *mut _,
        )
    } != -1;
    ok_or_ret_errno!(success => cred)
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
