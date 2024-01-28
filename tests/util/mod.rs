//! Test utilities for allocating an address for the server and then spawning clients to connect to
//! it.
#![allow(dead_code, unused_macros)]

mod choke;

mod drive;
#[macro_use]
mod eyre;
mod xorshift;
#[macro_use]
mod namegen;

#[allow(unused_imports)]
pub use {drive::*, eyre::*, namegen::*, xorshift::*};

#[cfg(feature = "tokio")]
pub mod tokio;

const NUM_CLIENTS: u32 = 80;
const NUM_CONCURRENT_CLIENTS: u32 = 6;

use color_eyre::eyre::WrapErr;
use std::{fmt::Arguments, io, sync::Arc};

pub fn testinit() {
    eyre::install();
}

pub fn message(msg: Option<Arguments<'_>>, server: bool, terminator: Option<char>) -> Box<str> {
    let msg = msg.unwrap_or_else(|| format_args!("Message"));
    let sc = if server { "server" } else { "client" };
    let mut msg = format!("{msg} from {sc}!");
    if let Some(t) = terminator {
        msg.push(t);
    }
    msg.into()
}

pub fn listen_and_pick_name<T>(
    namegen: &mut NameGen,
    mut bindfn: impl FnMut(&str) -> io::Result<T>,
) -> TestResult<(Arc<str>, T)> {
    use std::io::ErrorKind::*;
    namegen
        .find_map(|nm| {
            let l = match bindfn(&nm) {
                Ok(l) => l,
                Err(e) if matches!(e.kind(), AddrInUse | PermissionDenied) => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap() // Infinite iterator
        .context("listener bind failed")
}
