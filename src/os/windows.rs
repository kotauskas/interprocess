//! Windows-specific functionality for various interprocess communication primitives, as well as
//! Windows-specific ones.

pub mod named_pipe;
pub mod unnamed_pipe;
//pub mod mailslot;

mod path_conversion;
mod security_descriptor;
mod share_handle;

pub use {path_conversion::*, security_descriptor::*, share_handle::*};

mod file_handle;
pub(crate) mod local_socket {
	pub mod dispatch;
	pub mod name;

	// temporary
	pub(crate) use super::named_pipe::local_socket::tokio;
}

pub(crate) use file_handle::*;

mod c_wrappers;
mod misc;

use misc::*;
