#![cfg(windows)]
#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

mod bytes;
mod msg;

use std::sync::mpsc::Sender;
fn mk_server(
    f: impl (FnOnce(Sender<String>, u32, bool, bool) -> TestResult),
    recv: bool,
    send: bool,
) -> impl (FnOnce(Sender<String>, u32) -> TestResult) {
    move |snd, numc| (f)(snd, numc, recv, send)
}
fn mk_client(f: impl (Fn(&str, bool, bool) -> TestResult), recv: bool, send: bool) -> impl (Fn(&str) -> TestResult) {
    move |nm| (f)(nm, recv, send)
}

#[test]
fn named_pipe_bytes_bidir() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, true, true), mk_client(client, true, true))
}

#[test]
fn named_pipe_bytes_unidir_client_to_server() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, true, false), mk_client(client, false, true))
}
#[test]
fn named_pipe_bytes_unidir_server_to_client() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, false, true), mk_client(client, true, false))
}

#[test]
fn named_pipe_msg_bidir() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, true, true), mk_client(client, true, true))
}

#[test]
fn named_pipe_msg_unidir_client_to_server() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, true, false), mk_client(client, false, true))
}
#[test]
fn named_pipe_msg_unidir_server_to_client() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(mk_server(server, false, true), mk_client(client, true, false))
}
