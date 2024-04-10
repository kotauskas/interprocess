//! Local sockets implemented using Unix domain sockets.

mod listener;
mod stream;
pub use {listener::*, stream::*};

#[cfg(feature = "tokio")]
pub(crate) mod tokio {
	mod listener;
	mod stream;
	pub use {listener::*, stream::*};
}

use crate::local_socket::Name;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::{linux::net::SocketAddrExt, unix::ffi::OsStrExt};
use std::{io, os::unix::net::SocketAddr, path::Path};

#[allow(clippy::indexing_slicing)]
fn name_to_addr(name: Name<'_>) -> io::Result<SocketAddr> {
	let is_ns = name.is_namespaced();
	let name = name.into_raw_cow();
	if is_ns {
		#[cfg(any(target_os = "linux", target_os = "android"))]
		{
			return SocketAddr::from_abstract_name(name.as_bytes());
		}
		#[cfg(not(any(target_os = "linux", target_os = "android")))]
		{
			unreachable!();
		}
	}
	SocketAddr::from_pathname(Path::new(&name))
}

#[derive(Clone, Debug, Default)]
struct ReclaimGuard(Option<Name<'static>>);
impl ReclaimGuard {
	fn new(name: Name<'static>) -> Self {
		Self(if name.is_path() { Some(name) } else { None })
	}
	#[cfg_attr(not(feature = "tokio"), allow(dead_code))]
	fn take(&mut self) -> Self {
		Self(self.0.take())
	}
	fn forget(&mut self) {
		self.0 = None;
	}
}
impl Drop for ReclaimGuard {
	fn drop(&mut self) {
		if let Self(Some(name)) = self {
			if name.is_path() {
				let _ = std::fs::remove_file(name.raw());
			}
		}
	}
}
