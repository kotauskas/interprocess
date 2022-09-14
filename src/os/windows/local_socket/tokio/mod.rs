//! Adapter module, implements Tokio local sockets under Windows.

mod listener;
pub use listener::*;

mod stream;
pub use stream::*;
