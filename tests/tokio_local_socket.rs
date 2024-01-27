// TODO test various error conditions
// TODO test reunite in some shape or form
#![cfg(feature = "tokio")]

mod no_server;
mod stream;

use crate::{
    local_socket::NameTypeSupport,
    testutil::{self, testinit, TestResult},
};

async fn test_stream(nmspc: bool) -> TestResult {
    use stream::*;
    testinit();
    testutil::tokio::drive_server_and_multiple_clients(move |s, n| server(s, n, nmspc), client)
        .await
}

#[tokio::test]
async fn stream_file() -> TestResult {
    if NameTypeSupport::query().paths_supported() {
        test_stream(false).await?;
    }
    Ok(())
}
#[tokio::test]
async fn stream_namespaced() -> TestResult {
    if NameTypeSupport::query().namespace_supported() {
        test_stream(true).await?;
    }
    Ok(())
}

#[tokio::test]
async fn no_server_file() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(false).await?;
    }
    Ok(())
}
#[tokio::test]
async fn no_server_namespaced() -> TestResult {
    testinit();
    if NameTypeSupport::query().namespace_supported() {
        no_server::run_and_verify_error(true).await?;
    }
    Ok(())
}
