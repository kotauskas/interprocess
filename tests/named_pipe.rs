#![cfg(windows)]

mod bytes;
mod msg;

use crate::{os::windows::named_pipe::PipeListenerOptions, tests::util::*};
use std::{
    io,
    path::Path,
    sync::{mpsc::Sender, Arc},
};

#[test]
fn bytes_bidir() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_duplex, client_duplex)
}

#[test]
fn bytes_unidir_client_to_server() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_cts, client_cts)
}
#[test]
fn bytes_unidir_server_to_client() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_stc, client_stc)
}

#[test]
fn msg_bidir() -> TestResult {
    use msg::*;
    testinit();
    drive_server_and_multiple_clients(server_duplex, client_duplex)
}

#[test]
fn msg_unidir_client_to_server() -> TestResult {
    use msg::*;
    testinit();
    drive_server_and_multiple_clients(server_cts, client_cts)
}
#[test]
fn msg_unidir_server_to_client() -> TestResult {
    use msg::*;
    testinit();
    drive_server_and_multiple_clients(server_stc, client_stc)
}

fn drive_server<L>(
    id: &'static str,
    name_sender: Sender<Arc<str>>,
    num_clients: u32,
    mut createfn: impl (FnMut(PipeListenerOptions<'_>) -> io::Result<L>),
    mut acceptfn: impl FnMut(&mut L) -> TestResult,
) -> TestResult {
    let (name, mut listener) = listen_and_pick_name(&mut NameGen::new(id, false), |nm| {
        createfn(PipeListenerOptions::new().path(Path::new(nm)))
    })?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        acceptfn(&mut listener)?;
    }

    Ok(())
}
