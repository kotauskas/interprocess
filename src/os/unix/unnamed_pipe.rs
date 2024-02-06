use super::FdOps;
use crate::{
	unnamed_pipe::{UnnamedPipeRecver as PubRecver, UnnamedPipeSender as PubSender},
	Sealed,
};
use libc::c_int;
use std::{
	fmt::{self, Debug, Formatter},
	io,
	os::{
		fd::OwnedFd,
		unix::io::{AsRawFd, FromRawFd},
	},
};

pub(crate) fn pipe() -> io::Result<(PubSender, PubRecver)> {
	let (success, fds) = unsafe {
		let mut fds: [c_int; 2] = [0; 2];
		let result = libc::pipe(fds.as_mut_ptr());
		(result == 0, fds)
	};
	if success {
		let (w, r) = unsafe {
			// SAFETY: we just created both of those file descriptors, which means that neither of
			// them can be in use elsewhere.
			let w = OwnedFd::from_raw_fd(fds[1]);
			let r = OwnedFd::from_raw_fd(fds[0]);
			(w, r)
		};
		let w = PubSender(UnnamedPipeSender(FdOps(w)));
		let r = PubRecver(UnnamedPipeRecver(FdOps(r)));
		Ok((w, r))
	} else {
		Err(io::Error::last_os_error())
	}
}

pub(crate) struct UnnamedPipeRecver(FdOps);
impl Sealed for UnnamedPipeRecver {}
impl Debug for UnnamedPipeRecver {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("UnnamedPipeRecver")
			.field("fd", &self.0 .0.as_raw_fd())
			.finish()
	}
}
multimacro! {
	UnnamedPipeRecver,
	forward_rbv(FdOps, &),
	forward_sync_ref_read,
	forward_try_clone,
	forward_handle,
	derive_sync_mut_read,
}

pub(crate) struct UnnamedPipeSender(FdOps);
impl Sealed for UnnamedPipeSender {}
impl Debug for UnnamedPipeSender {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("UnnamedPipeSender")
			.field("fd", &self.0 .0.as_raw_fd())
			.finish()
	}
}

multimacro! {
	UnnamedPipeSender,
	forward_rbv(FdOps, &),
	forward_sync_ref_write,
	forward_try_clone,
	forward_handle,
	derive_sync_mut_write,
}
