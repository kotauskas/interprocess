//! Non-blocking wrappers for blocking interprocess communication primitives.
//!
//! Blocking is unacceptable in an async context, as it is the very problem that asynchrony aims to mitigate. This module contains wrappers for the base interprocess primitives, allowing their usage in an asyncronous runtime.
//!
//! The layout of this module aims to closely resemble the crate root, in that all the modules here mirror their blocking counterparts – check them out for usage examples and details about the differences you may encounter when porting blocking code to an async architecture.

mod imports;
pub mod local_socket;
