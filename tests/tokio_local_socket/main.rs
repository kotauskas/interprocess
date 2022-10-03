#![cfg(feature = "tokio_support")]
#[path = "../util/mod.rs"]
mod util;
use util::TestResult;

mod no_server;
mod stream;

use {interprocess::local_socket::NameTypeSupport, tokio::try_join};

#[tokio::test]
async fn tokio_local_socket_stream() -> TestResult {
    use stream::*;
    // If only one name type is supported, this one will choose the supported one. If both are
    // supported, this will try paths first.
    let f1 = util::tokio::drive_server_and_multiple_clients(|s, n| server(s, n, false), client);
    if NameTypeSupport::query() == NameTypeSupport::Both {
        // Try the namespace now.
        let f2 = util::tokio::drive_server_and_multiple_clients(|s, n| server(s, n, true), client);
        try_join!(f1, f2)?;
    } else {
        f1.await?;
    }
    Ok(())
}
#[tokio::test]
async fn tokio_local_socket_no_server() -> TestResult {
    // Same as above.
    let f1 = no_server::run_and_verify_error(false);
    if NameTypeSupport::query() == NameTypeSupport::Both {
        let f2 = no_server::run_and_verify_error(true);
        try_join!(f1, f2)?;
    } else {
        f1.await?;
    }
    Ok(())
}
