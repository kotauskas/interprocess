//! Asynchronous Ud-sockets which work with the Tokio runtime and event loop.
//!
//! The Tokio integration allows the Ud-socket streams and listeners to be notified by the OS kernel whenever they're ready to be read from of written to, instead of spawning threads just to put them in a wait state of blocking on the I/O.
//!
//! Types from this module will *not* work with other async runtimes, such as `async-std` or `smol`, since the Tokio types' methods will panic whenever they're called outside of a Tokio runtime context. Open an issue if you'd like to see other runtimes supported as well.

// contains macros, has to go before the other modules
#[macro_use]
mod util;

mod listener;
mod socket;
mod stream;
pub use {listener::*, socket::*, stream::*};

#[cfg(uds_supported)]
use super::c_wrappers;
