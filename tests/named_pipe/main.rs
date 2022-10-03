#![cfg(windows)]
#[path = "../util/mod.rs"]
mod util;

mod bytes;
mod bytes_unidir;
mod msg;
mod msg_unidir;

#[test]
fn named_pipe_bytes() {
    util::drive_server_and_multiple_clients(bytes::server, bytes::client)
}

#[test]
fn named_pipe_bytes_unidir() {
    util::drive_server_and_multiple_clients(bytes_unidir::server, bytes_unidir::client)
}

#[test]
fn named_pipe_msg() {
    util::drive_server_and_multiple_clients(msg::server, msg::client)
}

#[test]
fn named_pipe_msg_unidir() {
    util::drive_server_and_multiple_clients(msg_unidir::server, msg_unidir::client)
}
