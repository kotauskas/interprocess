//! Unix-specific functionality for various interprocess communication primitives, as well as Unix-specific ones.
//!
//! ## FIFO files
//! This type of interprocess communication similar to unnamed pipes in that they are unidirectional byte channels which behave like files. The difference is that FIFO files are actual (pseudo)files on the filesystem and thus can be accessed by unrelated applications (one doesn't need to be spawned by another).
//!
//! FIFO files are available on all supported systems.
//!
//! ## Unix domain sockets
//! Those are sockets used specifically for local IPC. They support bidirectional connections, identification by file path or inside the abstract Linux socket namespace, optional preservation of message boundaries (`SOCK_DGRAM` UDP-like interface) and transferring file descriptor ownership.
//!
//! Unix domain sockets are not available on ARM Newlib, but are supported on all other Unix-like systems.

#![cfg_attr(not(unix), allow(warnings))]

pub(crate) mod imports;

#[cfg(unix)]
mod fdops;
#[cfg(unix)]
// pub(self) is just a fancy way of saying priv (i.e. no access modifier), but
// we want to make it clear that we're exporting to child modules here rather
// than importing for use within this module.
pub(self) use fdops::*;

pub mod fifo_file;
#[cfg(any(doc, all(feature = "signals", se_basic)))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "signals")))]
pub mod signal;

#[cfg(any(doc, uds_supported))]
pub mod udsocket;

#[cfg(unix)]
pub(crate) mod local_socket;
#[cfg(unix)]
pub(crate) mod unnamed_pipe;
