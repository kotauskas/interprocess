#![cfg(windows)]
#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

mod bytes;
mod msg;

use interprocess::os::windows::named_pipe::PipeListenerOptions;
use std::{
    ffi::OsStr,
    io,
    sync::{mpsc::Sender, Arc},
};

#[test]
fn named_pipe_bytes_bidir() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_duplex, client_duplex)
}

#[test]
fn named_pipe_bytes_unidir_client_to_server() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_cts, client_cts)
}
#[test]
fn named_pipe_bytes_unidir_server_to_client() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_stc, client_stc)
}

#[test]
fn named_pipe_msg_bidir() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_duplex, client_duplex)
}

#[test]
fn named_pipe_msg_unidir_client_to_server() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_cts, client_cts)
}
#[test]
fn named_pipe_msg_unidir_server_to_client() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_stc, client_stc)
}

fn drive_server<L>(
    name_sender: Sender<Arc<str>>,
    num_clients: u32,
    mut createfn: impl (FnMut(PipeListenerOptions) -> io::Result<L>),
    mut acceptfn: impl FnMut(&mut L) -> TestResult,
) -> TestResult {
    let (name, mut listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), true), |nm| {
        createfn(PipeListenerOptions::new().name(nm.as_ref() as &OsStr))
    })?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        acceptfn(&mut listener)?;
    }

    Ok(())
}
