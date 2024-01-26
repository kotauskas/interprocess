#![doc = include_str!("../README.md")]
// TODO mailslots
// TODO add OS-specific ext-traits
// TODO un-mod.rs
// TODO make doctests not no_run
// TODO inspect panic points
// - **Mailslots** – Windows-specific interprocess communication primitive for short messages, potentially even across
// the network
#![cfg_attr(feature = "doc_cfg", feature(doc_cfg))]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![allow(clippy::nonstandard_macro_braces)]
#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(any(not(any(windows, unix)), target_os = "emscripten"))]
compile_error!(
    "Your target operating system is not supported by interprocess – check if yours is in the list of \
supported systems, and if not, please open an issue on the GitHub repository if you think that it should be included"
);

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!(
    "Platforms with exotic pointer widths (neither 32-bit nor 64-bit) are not supported by interprocess – \
if you think that your specific case needs to be accounted for, please open an issue on the GitHub repository"
);

#[macro_use]
mod macros;

pub mod local_socket;
pub mod unnamed_pipe;
//pub mod shared_memory;

pub mod error;

mod try_clone;
pub use try_clone::*;

mod misc;
pub(crate) use misc::*;

/// Platform-specific functionality for various interprocess communication primitives.
///
/// This module houses two modules: `unix` and `windows`, although only one at a time will be
/// visible, depending on which platform the documentation was built on. If you're using
/// [Docs.rs](https://docs.rs/interprocess/latest/interprocess), you can view the documentation for
/// Windows, macOS, Linux and FreeBSD using the Platform menu on the Docs.rs-specific header bar at
/// the top of the page. Docs.rs builds also have the nightly-only `doc_cfg` feature enabled by
/// default, with which everything platform-specific has a badge next to it which specifies the
/// `cfg(...)` conditions for that item to be available.
pub mod os {
    #[cfg(unix)]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
    pub mod unix;
    #[cfg(windows)]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
    pub mod windows;
}

#[cfg(any(doc, test))]
#[path = "../tests/util/mod.rs"]
#[macro_use]
mod testutil;

#[cfg(test)]
#[path = "../tests/index.rs"]
mod tests;
