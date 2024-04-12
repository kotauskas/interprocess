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

use crate::local_socket::{Name, NameInner};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::linux::net::SocketAddrExt;
use std::{io, os::unix::net::SocketAddr};

#[allow(clippy::indexing_slicing)]
fn name_to_addr(name: Name<'_>) -> io::Result<SocketAddr> {
	match name.0 {
		NameInner::UdSocketPath(path) => SocketAddr::from_pathname(path),
		#[cfg(any(target_os = "linux", target_os = "android"))]
		NameInner::UdSocketNs(name) => SocketAddr::from_abstract_name(name),
	}
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
		if let Self(Some(Name(NameInner::UdSocketPath(path)))) = self {
			let _ = std::fs::remove_file(path);
		}
	}
}
