use super::unixprelude::*;
#[allow(unused_imports)]
use crate::{FdOrErrno, OrErrno};
use libc::{sockaddr_un, AF_UNIX};
#[cfg(feature = "tokio")]
use std::net::Shutdown;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::linux::net::SocketAddrExt;
use std::{
	io,
	mem::{transmute, zeroed},
	os::unix::net::SocketAddr,
	sync::atomic::{AtomicBool, Ordering::Relaxed},
};

pub(super) unsafe fn fcntl_int(fd: BorrowedFd<'_>, cmd: c_int, val: c_int) -> io::Result<c_int> {
	let val = unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) };
	(val != -1).true_val_or_errno(val)
}

pub(super) fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		let new_fd = unsafe { fcntl_int(fd, libc::F_DUPFD_CLOEXEC, 0)? };
		Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
	}
	#[cfg(not(any(target_os = "linux", target_os = "android")))]
	{
		let new_fd = unsafe {
			libc::dup(fd.as_raw_fd())
				.fd_or_errno()
				.map(|fd| OwnedFd::from_raw_fd(fd))?
		};
		set_cloexec(new_fd.as_fd())?;
		Ok(new_fd)
	}
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn get_fdflags(fd: BorrowedFd<'_>) -> io::Result<c_int> {
	let val = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD, 0) };
	(val != -1).true_val_or_errno(val)
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_fdflags(fd: BorrowedFd<'_>, flags: c_int) -> io::Result<()> {
	unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags) != -1 }.true_val_or_errno(())
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
	set_fdflags(fd, get_fdflags(fd)? | libc::FD_CLOEXEC)?;
	Ok(())
}

pub(super) fn set_mode(fd: BorrowedFd<'_>, mode: mode_t) -> io::Result<()> {
	unsafe { libc::fchmod(fd.as_raw_fd(), mode) != -1 }.true_val_or_errno(())
}

static CAN_FCHMOD_SOCKETS: AtomicBool = AtomicBool::new(true);
pub(super) fn set_socket_mode(fd: BorrowedFd<'_>, mode: mode_t) -> io::Result<()> {
	if mode == 0o666 {
		return Ok(());
	}
	let rslt = set_mode(fd, mode);
	if let Err(e) = &rslt {
		if e.kind() == io::ErrorKind::InvalidInput {
			CAN_FCHMOD_SOCKETS.store(false, Relaxed);
		}
	}
	rslt
}

/// Creates a Unix domain socket of the given type. If on Linux or Android and `nonblocking` is
/// `true`, also makes it nonblocking.
pub(super) const CAN_CREATE_NONBLOCKING: bool =
	cfg!(any(target_os = "linux", target_os = "android"));
fn create_socket(ty: c_int, nonblocking: bool) -> io::Result<OwnedFd> {
	let flags = {
		#[cfg(not(any(target_os = "linux", target_os = "android")))]
		{
			0
		}
		#[cfg(any(target_os = "linux", target_os = "android"))]
		{
			let mut flags = libc::SOCK_CLOEXEC;
			if nonblocking {
				flags |= libc::SOCK_NONBLOCK;
			}
			flags
		}
	};
	let val = unsafe { libc::socket(AF_UNIX, ty | flags, 0) };
	let fd = (val != -1)
		.true_val_or_errno(val)
		.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })?;

	#[cfg(not(any(target_os = "linux", target_os = "android")))]
	{
		set_cloexec(fd.as_fd())?;
	}
	Ok(fd)
}

fn addr_to_slice(addr: &SocketAddr) -> (&[u8], usize) {
	if let Some(slice) = addr.as_pathname() {
		(slice.as_os_str().as_bytes(), 0)
	} else {
		#[cfg(any(target_os = "linux", target_os = "android"))]
		if let Some(slice) = addr.as_abstract_name() {
			return (slice, 1);
		}
		(&[], 0)
	}
}

const SUN_PATH_OFFSET: usize = unsafe {
	// This code may or may not have been copied from the standard library
	let addr = zeroed::<sockaddr_un>();
	let base = (&addr as *const sockaddr_un).cast::<i8>();
	let path = &addr.sun_path as *const c_char;
	path.byte_offset_from(base) as usize
};

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
fn bind(fd: BorrowedFd<'_>, addr: &SocketAddr) -> io::Result<()> {
	let (path, extra) = addr_to_slice(addr);
	let path = unsafe { transmute::<&[u8], &[i8]>(path) };

	let mut addr = unsafe { zeroed::<sockaddr_un>() };
	addr.sun_family = AF_UNIX as _;
	addr.sun_path[extra..(extra + path.len())].copy_from_slice(path);

	let len = path.len() + extra + SUN_PATH_OFFSET;

	unsafe {
		libc::bind(
			fd.as_raw_fd(),
			(&addr as *const sockaddr_un).cast(),
			// It's impossible for this to exceed socklen_t::MAX, since it came from a valid
			// SocketAddr
			len as _,
		) != -1
	}
	.true_val_or_errno(())
}

fn listen(fd: BorrowedFd<'_>) -> io::Result<()> {
	// The standard library does this
	#[cfg(any(
		target_os = "windows",
		target_os = "redox",
		target_os = "espidf",
		target_os = "horizon"
	))]
	const BACKLOG: libc::c_int = 128;
	#[cfg(any(
		target_os = "linux",
		target_os = "freebsd",
		target_os = "openbsd",
		target_os = "macos"
	))]
	const BACKLOG: libc::c_int = -1;
	#[cfg(not(any(
		target_os = "windows",
		target_os = "redox",
		target_os = "linux",
		target_os = "freebsd",
		target_os = "openbsd",
		target_os = "macos",
		target_os = "espidf",
		target_os = "horizon"
	)))]
	const BACKLOG: libc::c_int = libc::SOMAXCONN;
	unsafe { libc::listen(fd.as_raw_fd(), BACKLOG) != -1 }.true_val_or_errno(())
}

struct WithUmask {
	new: mode_t,
	old: mode_t,
}
impl WithUmask {
	pub fn set(new: mode_t) -> Self {
		Self {
			new,
			old: Self::umask(new),
		}
	}
	fn umask(mode: mode_t) -> mode_t {
		unsafe { libc::umask(mode) }
	}
}
impl Drop for WithUmask {
	fn drop(&mut self) {
		let expected_new = Self::umask(self.old);
		assert_eq!(self.new, expected_new, "concurrent umask use detected");
	}
}

pub(super) fn bind_and_listen_with_mode(
	ty: c_int,
	addr: &SocketAddr,
	nonblocking: bool,
	mode: mode_t,
) -> io::Result<OwnedFd> {
	if mode & 0o111 != 0 {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"sockets can not be marked executable",
		));
	}

	if CAN_FCHMOD_SOCKETS.load(Relaxed) {
		let sock = create_socket(ty, nonblocking)?;
		match set_socket_mode(sock.as_fd(), mode) {
			Ok(()) => {
				bind(sock.as_fd(), addr)?;
				listen(sock.as_fd())?;
				return Ok(sock);
			}
			Err(e) if e.kind() == io::ErrorKind::InvalidInput => {}
			Err(e) => return Err(e),
		}
	}
	// Sad path, either got false or fell through from the second match arm
	let _dg = (mode != 0o666).then(|| {
		// The value that permissions get ANDed with is actually the inverse of the umask
		let umask = !mode & 0o777;
		WithUmask::set(umask)
	});
	// We race in this muthafucka, better get yo secure code ass back to Linux
	let sock = create_socket(ty, nonblocking)?;
	bind(sock.as_fd(), addr)?;
	listen(sock.as_fd())?;
	Ok(sock)
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
