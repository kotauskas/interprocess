use crate::{
	local_socket::{traits::Stream as _, ListenerOptions, Stream},
	os::unix::local_socket::ListenerOptionsExt,
	tests::util::*,
	OrErrno,
};
use libc::mode_t;
use std::{mem::zeroed, os::unix::prelude::*, sync::Arc};

fn get_mode(fd: BorrowedFd<'_>) -> TestResult<mode_t> {
	let mut stat = unsafe { zeroed::<libc::stat>() };
	unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) != -1 }
		.true_val_or_errno(())
		.opname("stat")?;
	Ok(stat.st_mode & 0o777)
}

fn test_inner(path: bool) -> TestResult {
	const MODE: libc::mode_t = 0o600;
	let (name, listener) =
		listen_and_pick_name(&mut namegen_local_socket(make_id!(), path), |nm| {
			ListenerOptions::new()
				.name(nm.borrow())
				.mode(MODE)
				.create_sync()
		})?;
	let _ = Stream::connect(Arc::try_unwrap(name).unwrap()).opname("client connect")?;
	ensure_eq!(get_mode(listener.as_fd())?, MODE);

	Ok(())
}

#[test]
fn local_socket_file_mode() -> TestResult {
	test_inner(true)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn local_socket_namespaced_mode() -> TestResult {
	test_inner(false)
}
