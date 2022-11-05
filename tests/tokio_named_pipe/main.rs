#![cfg(all(windows, feature = "tokio_support"))]
#[path = "../util/mod.rs"]
mod util;

mod bytes;
mod bytes_unidir_client_to_server;
mod bytes_unidir_server_to_client;
mod msg;
mod msg_unidir_client_to_server;
/*mod msg_unidir_server_to_client;*/

use util::TestResult;

#[tokio::test]
async fn tokio_named_pipe_bytes() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(bytes::server, bytes::client).await
}

#[tokio::test]
async fn tokio_named_pipe_bytes_unidir_client_to_server() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(
        bytes_unidir_client_to_server::server,
        bytes_unidir_client_to_server::client,
    )
    .await
}
#[tokio::test]
async fn tokio_named_pipe_bytes_unidir_server_to_client() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(
        bytes_unidir_server_to_client::server,
        bytes_unidir_server_to_client::client,
    )
    .await
}

#[tokio::test]
async fn tokio_named_pipe_msg() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(msg::server, msg::client).await
}

#[tokio::test]
async fn tokio_named_pipe_msg_unidir_client_to_server() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(
        msg_unidir_client_to_server::server,
        msg_unidir_client_to_server::client,
    )
    .await
}
/*#[tokio::test]
async fn tokio_named_pipe_msg_unidir_server_to_client() -> TestResult {
    util::tokio::drive_server_and_multiple_clients(
        msg_unidir_server_to_client::server,
        msg_unidir_server_to_client::client,
    )
    .await
}*/
