#![cfg(windows)]
#[path = "../util/mod.rs"]
mod util;

mod bytes;
mod bytes_unidir_client_to_server;
mod bytes_unidir_server_to_client;
mod msg;
mod msg_unidir_client_to_server;
mod msg_unidir_server_to_client;

#[test]
fn named_pipe_bytes() {
    util::drive_server_and_multiple_clients(bytes::server, bytes::client)
}

#[test]
fn named_pipe_bytes_unidir_client_to_server() {
    util::drive_server_and_multiple_clients(
        bytes_unidir_client_to_server::server,
        bytes_unidir_client_to_server::client,
    )
}
#[test]
fn named_pipe_bytes_unidir_server_to_client() {
    util::drive_server_and_multiple_clients(
        bytes_unidir_server_to_client::server,
        bytes_unidir_server_to_client::client,
    )
}

#[test]
fn named_pipe_msg() {
    util::drive_server_and_multiple_clients(msg::server, msg::client)
}

#[test]
fn named_pipe_msg_unidir_client_to_server() {
    util::drive_server_and_multiple_clients(
        msg_unidir_client_to_server::server,
        msg_unidir_client_to_server::client,
    )
}
#[test]
fn named_pipe_msg_unidir_server_to_client() {
    util::drive_server_and_multiple_clients(
        msg_unidir_server_to_client::server,
        msg_unidir_server_to_client::client,
    )
}
