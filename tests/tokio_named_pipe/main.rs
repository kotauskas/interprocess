#![cfg(all(windows, feature = "tokio_support"))]
#[path = "../util/mod.rs"]
mod util;

mod basic_bytes;
mod basic_bytes_unidir;
mod basic_msg;

use util::TestResult;

#[tokio::test]
async fn tokio_named_pipe_basic_bytes() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(basic_bytes::server, basic_bytes::client).await
}

#[tokio::test]
async fn tokio_named_pipe_basic_bytes_unidir() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(
        basic_bytes_unidir::server,
        basic_bytes_unidir::client,
    )
    .await
}
