#![cfg(all(windows, feature = "tokio_support"))]
#[path = "../util/mod.rs"]
mod util;

mod basic_bytes;

use util::TestResult;

#[tokio::test]
async fn tokio_named_pipe_basic_bytes() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(basic_bytes::server, basic_bytes::client).await
}
