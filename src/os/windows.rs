//! Windows-specific functionality for various interprocess communication primitives, as well as
//! Windows-specific ones.
#![cfg_attr(not(windows), allow(warnings))]

pub mod named_pipe;
pub mod unnamed_pipe;
//pub mod mailslot;

mod share_handle;

pub use share_handle::*;

mod c_wrappers;
pub mod security_descriptor;
mod file_handle;
pub(crate) mod local_socket;
mod misc;

pub(crate) use file_handle::*;
use misc::*;
