//! Adapter module, implements local sockets under Windows.

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

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNs;
pub fn name_type_support_query() -> NameTypeSupport {
	NAME_TYPE_ALWAYS_SUPPORTED
}
pub fn is_namespaced(_: &LocalSocketName<'_>) -> bool {
	true
}
