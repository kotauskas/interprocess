#![cfg(all(windows, feature = "tokio_support"))]
#[path = "../util/mod.rs"]
mod util;

mod bytes;
mod bytes_unidir;
mod msg;
mod msg_unidir;

use util::TestResult;

#[tokio::test]
async fn tokio_named_pipe_bytes() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(bytes::server, bytes::client).await
}

#[tokio::test]
async fn tokio_named_pipe_bytes_unidir() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(bytes_unidir::server, bytes_unidir::client).await
}

#[tokio::test]
async fn tokio_named_pipe_msg() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(msg::server, msg::client).await
}

#[tokio::test]
async fn tokio_named_pipe_msg_unidir() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(msg_unidir::server, msg_unidir::client).await
}
