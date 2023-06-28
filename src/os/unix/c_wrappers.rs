use super::unixprelude::*;
use std::io;

pub(super) unsafe fn fcntl_int(fd: BorrowedFd<'_>, cmd: c_int, val: c_int) -> io::Result<c_int> {
    let val = unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) };
    ok_or_ret_errno!(val != -1 => val)
}
pub(super) unsafe fn fcntl_noarg(fd: BorrowedFd<'_>, cmd: c_int) -> io::Result<c_int> {
    let val = unsafe { libc::fcntl(fd.as_raw_fd(), cmd) };
    ok_or_ret_errno!(val != -1 => val)
}

pub(super) fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    #[cfg(target_os = "linux")]
    {
        let new_fd = unsafe { fcntl_int(fd, libc::F_DUPFD_CLOEXEC, 0)? };
        Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let (val, success) = unsafe {
            let ret = libc::dup(fd.as_raw_fd());
            (ret, ret != -1)
        };
        let new_fd = ok_or_ret_errno!(success => unsafe { OwnedFd::from_raw_fd(val) })?;
        set_cloexec(new_fd.as_fd())?;
        Ok(new_fd)
    }
}

pub(super) fn get_fdflags(fd: BorrowedFd<'_>) -> io::Result<i32> {
    let (val, success) = unsafe {
        let ret = libc::fcntl(fd.as_raw_fd(), libc::F_GETFD, 0);
        (ret, ret != -1)
    };
    ok_or_ret_errno!(success => val)
}
pub(super) fn set_fdflags(fd: BorrowedFd<'_>, flags: i32) -> io::Result<()> {
    let success = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags) != -1 };
    ok_or_ret_errno!(success => ())
}
pub(super) fn set_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
    set_fdflags(fd, get_fdflags(fd)? | libc::FD_CLOEXEC)?;
    Ok(())
}

pub(super) fn get_uid(ruid: bool) -> uid_t {
    unsafe {
        if ruid {
            libc::getuid()
        } else {
            libc::geteuid()
        }
    }
}
pub(super) fn get_gid(rgid: bool) -> gid_t {
    unsafe {
        if rgid {
            libc::getgid() // more like git gud am i right
        } else {
            libc::getegid()
        }
    }
}
pub(super) fn get_pid() -> pid_t {
    unsafe { libc::getpid() }
}
