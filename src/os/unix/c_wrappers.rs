use super::unixprelude::*;
use std::{io, net::Shutdown};

pub(super) unsafe fn fcntl_int(fd: BorrowedFd<'_>, cmd: c_int, val: c_int) -> io::Result<c_int> {
    let val = unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) };
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

#[cfg(feature = "tokio")]
pub(super) fn shutdown(fd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
    let how = match how {
        Shutdown::Read => libc::SHUT_RD,
        Shutdown::Write => libc::SHUT_WR,
        Shutdown::Both => libc::SHUT_RDWR,
    };
    let success = unsafe { libc::shutdown(fd.as_raw_fd(), how) != -1 };
    ok_or_ret_errno!(success => ())
}
