//! Windows-specific functionality for various interprocess communication primitives, as well as
//! Windows-specific ones.
#![cfg_attr(not(windows), allow(warnings))]

pub mod named_pipe;
pub mod unnamed_pipe;
//pub mod mailslot;

mod security_descriptor;
mod share_handle;
pub use {security_descriptor::*, share_handle::*};

mod file_handle;
pub(crate) mod local_socket;

pub(crate) use file_handle::*;

mod c_wrappers;
mod misc;

use misc::*;
