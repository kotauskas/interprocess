//! Adapter module, implements local sockets under Unix.

mod listener;
mod stream;
pub use {listener::*, stream::*};

pub mod to_name;

#[cfg(feature = "tokio")]
pub mod tokio {
	mod listener;
	mod stream;
	pub use {listener::*, stream::*};
}

use crate::local_socket::{LocalSocketName, NameTypeSupport};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::linux::net::SocketAddrExt;
use std::{
	io,
	os::unix::{ffi::OsStrExt, net::SocketAddr},
	path::Path,
};

#[allow(clippy::indexing_slicing)]
fn name_to_addr(name: LocalSocketName<'_>) -> io::Result<SocketAddr> {
	let _is_ns = name.is_namespaced();
	let name = name.into_raw_cow();
	#[cfg(any(target_os = "linux", target_os = "android"))]
	if _is_ns {
		let mut bytes = name.as_bytes();
		if bytes.first() == Some(&b'\0') {
			bytes = &bytes[1..];
		}
		return SocketAddr::from_abstract_name(bytes);
	}
	SocketAddr::from_pathname(Path::new(&name))
}

#[derive(Clone, Debug, Default)]
struct ReclaimGuard(Option<LocalSocketName<'static>>);
impl ReclaimGuard {
	fn new(name: LocalSocketName<'static>) -> Self {
		Self(if name.is_path() { Some(name) } else { None })
	}
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
			if name.is_namespaced() {
				let _ = std::fs::remove_file(name.raw());
			}
		}
	}
}

pub fn name_type_support_query() -> NameTypeSupport {
	NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(uds_linux_namespace)]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(uds_linux_namespace))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyFs;

pub fn is_namespaced(slf: &LocalSocketName<'_>) -> bool {
	!slf.is_path()
}
