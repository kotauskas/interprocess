//! Platform-specific functionality for various interprocess communication primitives.
//!
//! This module houses two modules: [`unix`] and [`windows`]. Depending on your platform, one of those is available, so if you only see one module here, don't worry — it's just not available for the platform on which you're browsing the docs. If you're using [Docs.rs], you can see the documentation for other platforms using the navigation bar on the top of the page.
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