//! Platform-specific functionality for various interprocess communication primitives.
//!
//! This module houses two modules: [`unix`](self::unix) and [`windows`](self::windows), although only one at a time will be visible, depending on which platform the documentation was built on. If you're using [Docs.rs](https://docs.rs/interprocess/latest/interprocess), you can view the documentation for Windows, macOS and Linux using the Platform menu on the Docs.rs-specific header bar at the top of the page. Docs.rs builds also have the nightly-only `doc_cfg` feature enabled by default, with which everything platform-specific has a badge next to it which specifies the `cfg(...)` conditions for that item to be available.

#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
pub mod unix;
#[cfg(windows)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
pub mod windows;
