#![cfg(windows)]
#[path = "../util/mod.rs"]
mod util;

mod basic_bytes;
mod basic_bytes_unidir;
mod basic_msg;
mod basic_msg_unidir;

#[test]
fn named_pipe_basic_bytes() {
    util::drive_server_and_multiple_clients(basic_bytes::server, basic_bytes::client)
}

#[test]
fn named_pipe_basic_bytes_unidir() {
    util::drive_server_and_multiple_clients(basic_bytes_unidir::server, basic_bytes_unidir::client)
}

#[test]
fn named_pipe_basic_msg() {
    util::drive_server_and_multiple_clients(basic_msg::server, basic_msg::client)
}

#[test]
fn named_pipe_basic_msg_unidir() {
    util::drive_server_and_multiple_clients(basic_msg_unidir::server, basic_msg_unidir::client)
}
