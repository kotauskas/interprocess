#![cfg(windows)]
#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

mod bytes;
mod bytes_unidir_client_to_server;
mod bytes_unidir_server_to_client;
mod msg;
mod msg_unidir_client_to_server;
mod msg_unidir_server_to_client;

#[test]
fn named_pipe_bytes() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}

#[test]
fn named_pipe_bytes_unidir_client_to_server() -> TestResult {
    use bytes_unidir_client_to_server::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}
#[test]
fn named_pipe_bytes_unidir_server_to_client() -> TestResult {
    use bytes_unidir_server_to_client::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}

#[test]
fn named_pipe_msg() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}

#[test]
fn named_pipe_msg_unidir_client_to_server() -> TestResult {
    use msg_unidir_client_to_server::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}
#[test]
fn named_pipe_msg_unidir_server_to_client() -> TestResult {
    use msg_unidir_server_to_client::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server, client)
}
