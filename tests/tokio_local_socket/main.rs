#![cfg(feature = "tokio")]
#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::{testinit, TestResult};

mod no_server;
mod stream;

use interprocess::local_socket::NameTypeSupport;

async fn tokio_local_socket_stream(nmspc: bool) -> TestResult {
    use stream::*;
    testinit();
    util::tokio::drive_server_and_multiple_clients(move |s, n| server(s, n, nmspc), client).await
}

#[tokio::test]
async fn tokio_local_socket_stream_file() -> TestResult {
    if NameTypeSupport::query().paths_supported() {
        tokio_local_socket_stream(false).await?;
    }
    Ok(())
}
#[tokio::test]
async fn tokio_local_socket_stream_namespaced() -> TestResult {
    if NameTypeSupport::query().namespace_supported() {
        tokio_local_socket_stream(true).await?;
    }
    Ok(())
}

#[tokio::test]
async fn tokio_local_socket_no_server_file() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(false).await?;
    }
    Ok(())
}
#[tokio::test]
async fn tokio_local_socket_no_server_namespaced() -> TestResult {
    testinit();
    if NameTypeSupport::query().namespace_supported() {
        no_server::run_and_verify_error(true).await?;
    }
    Ok(())
}
