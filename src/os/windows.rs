//! Windows-specific functionality for various interprocess communication primitives, as well as
//! Windows-specific ones.

pub mod local_socket;
pub mod named_pipe;
pub mod security_descriptor;
pub mod unnamed_pipe;
//pub mod mailslot;

mod path_conversion;
mod share_handle;

pub use {path_conversion::*, share_handle::*};

mod file_handle;
mod limbo_pool;
pub(crate) mod misc;
#[cfg(feature = "tokio")]
mod needs_flush;
mod sync_pipe_limbo;
#[cfg(feature = "tokio")]
mod tokio_flusher;
pub(crate) use {file_handle::*, misc::*, needs_flush::*, tokio_flusher::*};

mod c_wrappers;
