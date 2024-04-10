//! Unix-specific local socket features.

pub(crate) mod dispatch_sync;
#[cfg(feature = "tokio")]
pub(crate) mod dispatch_tokio;
pub(crate) mod name_type;

pub use name_type::*;

use crate::{local_socket::ListenerOptions, Sealed};

/// Unix-specific [listener options](ListenerOptions).
#[allow(private_bounds)]
pub trait ListenerOptionsExt: Sized + Sealed {
	/// Sets the file mode (Unix permissions) to be applied to the socket file.
	///
	/// Note that this *may* or *may not* obey `umask`. It is recommended to set `umask` to 666₈
	/// just before `.create()`.
	#[must_use = builder_must_use!()]
	fn mode(self, mode: libc::mode_t) -> Self;
}

impl ListenerOptionsExt for ListenerOptions<'_> {
	#[inline(always)]
	fn mode(mut self, mode: libc::mode_t) -> Self {
		self.mode = mode;
		self
	}
}
