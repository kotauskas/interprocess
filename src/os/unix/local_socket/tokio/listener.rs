use super::Stream;
use crate::{
	local_socket::Name,
	os::unix::local_socket::{listener::Listener as SyncListener, ReclaimGuard},
};
use std::{
	fmt::{self, Debug, Formatter},
	io,
	os::unix::prelude::*,
};
use tokio::net::UnixListener;

pub struct Listener {
	listener: UnixListener,
	reclaim: ReclaimGuard,
}
impl Listener {
	pub fn bind(name: Name<'_>, keep_name: bool) -> io::Result<Self> {
		Self::try_from(SyncListener::bind(name, keep_name)?)
	}
	pub async fn accept(&self) -> io::Result<Stream> {
		let inner = self.listener.accept().await?.0;
		Ok(Stream(inner))
	}

	pub fn do_not_reclaim_name_on_drop(&mut self) {
		self.reclaim.forget();
	}
}

impl TryFrom<SyncListener> for Listener {
	type Error = io::Error;
	fn try_from(mut sync: SyncListener) -> io::Result<Self> {
		sync.set_nonblocking(true)?;
		let reclaim = sync.reclaim.take();
		Ok(Self {
			listener: UnixListener::from_std(sync.into())?,
			reclaim,
		})
	}
}

impl Debug for Listener {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("Listener")
			.field("fd", &self.listener.as_raw_fd())
			.field("reclaim", &self.reclaim)
			.finish()
	}
}
impl AsFd for Listener {
	#[inline]
	fn as_fd(&self) -> BorrowedFd<'_> {
		self.listener.as_fd()
	}
}
impl TryFrom<Listener> for OwnedFd {
	type Error = io::Error;
	fn try_from(mut slf: Listener) -> io::Result<Self> {
		slf.listener.into_std().map(|s| {
			slf.reclaim.forget();
			s.into()
		})
	}
}
impl TryFrom<OwnedFd> for Listener {
	// TODO use FromFdError
	type Error = io::Error;
	fn try_from(fd: OwnedFd) -> io::Result<Self> {
		Self::try_from(SyncListener::from(fd))
	}
}
