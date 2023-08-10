//! Test utilities for allocating an address for the server and then spawning clients to connect to it.
#![allow(dead_code, unused_macros)]

mod choke;

mod drive;
#[macro_use]
mod eyre;
mod xorshift;
#[macro_use]
mod namegen;
pub use {drive::*, eyre::*, namegen::*, xorshift::*};

#[cfg(feature = "tokio")]
pub mod tokio;

const NUM_CLIENTS: u32 = 80;
const NUM_CONCURRENT_CLIENTS: u32 = 6;

pub fn message(server: bool, terminator: Option<char>) -> String {
    let sc = if server { "server" } else { "client" };
    let mut msg = format!("Message from {sc}!");
    if let Some(t) = terminator {
        msg.push(t);
    }
    msg
}
