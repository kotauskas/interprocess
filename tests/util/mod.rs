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

use color_eyre::eyre::Context;
use std::io;

pub fn message(server: bool, terminator: Option<char>) -> String {
    let sc = if server { "server" } else { "client" };
    let mut msg = format!("Message from {sc}!");
    if let Some(t) = terminator {
        msg.push(t);
    }
    msg
}

pub fn listen_and_pick_name<T>(
    namegen: &mut NameGen,
    mut bindfn: impl FnMut(&str) -> io::Result<T>,
) -> TestResult<(String, T)> {
    namegen
        .find_map(|nm| {
            let l = match bindfn(&nm) {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("listener bind failed")
}
