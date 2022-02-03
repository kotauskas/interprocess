//! Platform-specific functionality for various interprocess communication primitives.
//!
//! This module houses two modules: [`unix`] and [`windows`]. Modules and items for foreign platforms are visible even if they're not available on your platform, so watch out. If you're using [Docs.rs], which enables the nightly-only `doc_cfg` feature by default, everything platform-specific will have a badge next to it which specifies the `cfg(...)` conditions for that item to be available.
//!
//! [`unix`]: mod.unix.html " "
//! [`windows`]: mod.windows.html " "
//! [Docs.rs]: https://Docs.rs/ " "

#[cfg(any(unix, doc))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
pub mod unix;
#[cfg(any(windows, doc))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
pub mod windows;
