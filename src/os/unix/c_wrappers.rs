use super::unixprelude::*;
use libc::{FD_CLOEXEC, F_GETFD, F_SETFD};
use std::io;

pub(super) fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let (val, success) = unsafe {
        let ret = libc::dup(fd.as_raw_fd());
        (ret, ret != -1)
    };
    let new_fd = ok_or_ret_errno!(success => unsafe { OwnedFd::from_raw_fd(val) })?;
    set_cloexec(new_fd.as_fd(), true)?;
    Ok(new_fd)
}

pub(super) fn get_fdflags(fd: BorrowedFd<'_>) -> io::Result<i32> {
    let (val, success) = unsafe {
        let ret = libc::fcntl(fd.as_raw_fd(), F_GETFD, 0);
        (ret, ret != -1)
    };
    ok_or_ret_errno!(success => val)
}
pub(super) fn set_fdflags(fd: BorrowedFd<'_>, flags: i32) -> io::Result<()> {
    let success = unsafe { libc::fcntl(fd.as_raw_fd(), F_SETFD, flags) != -1 };
    ok_or_ret_errno!(success => ())
}
pub(super) fn set_cloexec(fd: BorrowedFd<'_>, cloexec: bool) -> io::Result<()> {
    let mut flags = get_fdflags(fd)? & (!FD_CLOEXEC); // Mask out cloexec to set it to a new value
    if cloexec {
        flags |= FD_CLOEXEC;
    }
    set_fdflags(fd, flags)?;
    Ok(())
}
