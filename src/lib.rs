//! Interprocess communication toolkit for Rust programs. The crate aims to expose as many platform-specific features as possible while maintaining a uniform interface for all platforms.
//!
//! # Features
//! ## Interprocess communication primitives
//! `interprocess` provides both OS-specific interfaces for IPC and cross-platform abstractions for them.
//!
//! ### Cross-platform IPC APIs
//! - **Local sockets** – similar to TCP sockets, but use filesystem or namespaced paths instead of ports on `localhost`, depending on the OS, bypassing the network stack entirely; implemented using named pipes on Windows and Unix domain sockets on Unix
//!
//! ### Platform-specific, but present on both Unix-like systems and Windows
//! - **Unnamed pipes** – anonymous file-like objects for communicating privately in one direction, most commonly used to communicate between a child process and its parent
//! - **Signals** – C signals on Windows, POSIX signals on Unix-like OSes *(deprecated)*
//!
//! ### Unix-only
//! - **FIFO files** – special type of file which is similar to unnamed pipes but exists on the filesystem, often referred to as "named pipes" but completely different from Windows named pipes
//! - **Unix domain sockets** – a type of socket which is built around the standard networking APIs but uses filesystem paths instead of ports on `localhost`, optionally using a spearate namespace on Linux akin to Windows named pipes
//!
//! ### Windows-only
//! - **Named pipes** – closely resembles Unix domain sockets, uses a separate namespace instead of on-drive paths
//!
//! ## Asynchronous I/O
//! Currently, only Tokio for local sockets, Unix domain sockets and Windows named pipes is supported. Support for `async-std` is planned.
//!
//! # Feature gates
//! - **`signals`**, *on* by default – enables support for POSIX signals and C signals. Pulls in additional dependencies.
//! - **`tokio_support`**, *off* by default – enables support for Tokio-powered efficient asynchronous IPC. Cannot simply be named `tokio` because of Cargo limitations.
//! - **`nonblocking`**, *on* by default – deprecated and will be removed, do not use.
//!
//! # License
//! This crate, along with all community contributions made to it, is dual-licensed under the terms of either the [MIT license] or the [Apache 2.0 license].
//!
//! [MIT license]: https://choosealicense.com/licenses/mit/ " "
//! [Apache 2.0 license]: https://choosealicense.com/licenses/apache-2.0/ " "
// TODO mailslots
// TODO shared memory
// TODO use standard library raw+owned FDs and handles
// TODO the Intra Doc Link Sweep
// - **Mailslots** – Windows-specific interprocess communication primitive for short messages, potentially even across the network
// - **Shared memory** – exposes a nice safe interface for shared memory based on mapping identifiers, with some additional platform-specific extensions

#![cfg_attr(feature = "doc_cfg", feature(doc_cfg))]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![allow(clippy::nonstandard_macro_braces)]
#![cfg_attr(
    unsafe_op_in_unsafe_fn_stable, // This is set by the build script on Rust 1.52+
    forbid(unsafe_op_in_unsafe_fn),
)]
#![cfg_attr(not(unsafe_op_in_unsafe_fn_stable), allow(unused_unsafe))]

// If an operating system is not listed here, the `compile_error!` is invoked
#[cfg(not(any(
    // "Linux-like" (src/unix/linux_like/mod.rs in libc)
    target_os = "linux",
    target_os = "android",
    target_os = "emscripten",

    // Windows. There is just one.
    target_os = "windows",

    // "BSD-like" (src/unix/bsd/mod.rs in libc)
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",

    // "Solarish" (src/unix/solarish/mod.rs in libc)
    target_os = "solaris",
    target_os = "illumos",

    // Haiku (src/unix/haiku/mod.rs in libc)
    target_os = "haiku",

    // Hermit (src/unix/hermit/mod.rs in libc)
    target_os = "hermit",

    // Redox (src/unix/redox/mod.rs in libc)
    target_os = "redox",
)))]
compile_error!("Your target operating system is not supported by interprocess – check if yours is in the list of supported systems, and if not, please open an issue on the GitHub repository if you think that it should be included");

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("Platforms with exotic pointer widths (neither 32-bit nor 64-bit) are not supported by interprocess – if you think that your specific case needs to be accounted for, please open an issue on the GitHub repository");

#[macro_use]
mod macros;

pub mod local_socket;
#[cfg(any(doc, feature = "nonblocking"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "nonblocking")))]
#[deprecated(note = "\
does not integrate with async runtimes, leading to poor performance and bugs related to reading \
and writing at the same time (you can't) – see the `tokio` modules for relevant IPC primitives \
or open an issue if you want more async runtimes to be supported as well")]
pub mod nonblocking;
pub mod unnamed_pipe;
//pub mod shared_memory;

pub mod os;

mod sealed;
pub(crate) use sealed::Sealed;

mod reliable_read_msg;
pub use reliable_read_msg::*;
