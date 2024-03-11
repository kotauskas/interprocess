use super::unixprelude::*;
#[allow(unused_imports)]
use crate::{FdOrErrno, OrErrno};
use std::{io, net::Shutdown};

pub(super) unsafe fn fcntl_int(fd: BorrowedFd<'_>, cmd: c_int, val: c_int) -> io::Result<c_int> {
	let val = unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) };
	(val != -1).true_val_or_errno(val)
}

pub(super) fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
	#[cfg(target_os = "linux")]
	{
		let new_fd = unsafe { fcntl_int(fd, libc::F_DUPFD_CLOEXEC, 0)? };
		Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
	}
	#[cfg(not(target_os = "linux"))]
	{
		let new_fd =
			unsafe { libc::dup(fd.as_raw_fd()).fd_or_errno(|| OwnedFd::from_raw_fd(new_fd))? };
		set_cloexec(new_fd.as_fd())?;
		Ok(new_fd)
	}
}

#[cfg(not(target_os = "linux"))]
fn get_fdflags(fd: BorrowedFd<'_>) -> io::Result<i32> {
	let val = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD, 0) };
	(val != -1).true_val_or_errno(val)
}
#[cfg(not(target_os = "linux"))]
fn set_fdflags(fd: BorrowedFd<'_>, flags: i32) -> io::Result<()> {
	unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags) != -1 }.true_val_or_errno(())
}
#[cfg(not(target_os = "linux"))]
fn set_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
	set_fdflags(fd, get_fdflags(fd)? | libc::FD_CLOEXEC)?;
	Ok(())
}

#[cfg(feature = "tokio")]
pub(super) fn shutdown(fd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
	let how = match how {
		Shutdown::Read => libc::SHUT_RD,
		Shutdown::Write => libc::SHUT_WR,
		Shutdown::Both => libc::SHUT_RDWR,
	};
	unsafe { libc::shutdown(fd.as_raw_fd(), how) != -1 }.true_val_or_errno(())
}
