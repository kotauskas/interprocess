use super::name_to_addr;
use crate::{error::ReuniteError, local_socket::LocalSocketName, TryClone};
use std::{io, os::unix::net::UnixStream, sync::Arc};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) UnixStream);
impl LocalSocketStream {
	pub fn connect(name: LocalSocketName<'_>) -> io::Result<Self> {
		UnixStream::connect_addr(&name_to_addr(name)?).map(Self)
	}
	#[inline]
	pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		self.0.set_nonblocking(nonblocking)
	}
	#[inline]
	pub fn split(self) -> (RecvHalf, SendHalf) {
		let arc = Arc::new(self);
		(RecvHalf(Arc::clone(&arc)), SendHalf(arc))
	}
	#[inline]
	pub fn reunite(rh: RecvHalf, sh: SendHalf) -> Result<Self, ReuniteError<RecvHalf, SendHalf>> {
		if !Arc::ptr_eq(&rh.0, &sh.0) {
			return Err(ReuniteError { rh, sh });
		}
		drop(rh);
		let inner = Arc::into_inner(sh.0).unwrap();
		Ok(inner)
	}
}

impl TryClone for LocalSocketStream {
	#[inline]
	fn try_clone(&self) -> std::io::Result<Self> {
		self.0.try_clone().map(Self)
	}
}

multimacro! {
	LocalSocketStream,
	forward_rbv(UnixStream, &),
	forward_sync_ref_rw,
	forward_handle(unix),
	derive_sync_mut_rw,
}

#[derive(Debug)]
pub struct RecvHalf(pub(super) Arc<LocalSocketStream>);
multimacro! {
	RecvHalf,
	forward_rbv(LocalSocketStream, *),
	forward_sync_ref_read,
	forward_as_handle,
	derive_sync_mut_read,
}

#[derive(Debug)]
pub struct SendHalf(pub(super) Arc<LocalSocketStream>);
multimacro! {
	SendHalf,
	forward_rbv(LocalSocketStream, *),
	forward_sync_ref_write,
	forward_as_handle,
	derive_sync_mut_write,
}
