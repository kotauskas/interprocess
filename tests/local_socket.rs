// TODO test various error conditions
// TODO test reunite in some shape or form

mod no_server;
mod stream;

use crate::{local_socket::NameTypeSupport, testutil::*};

fn test_stream(nmspc: bool) -> TestResult {
    use stream::*;
    testinit();
    drive_server_and_multiple_clients(|s, n| server(s, n, nmspc), client)?;
    Ok(())
}

#[test]
fn stream_file() -> TestResult {
    if NameTypeSupport::query().paths_supported() {
        test_stream(false)?;
    }
    Ok(())
}
#[test]
fn stream_namespaced() -> TestResult {
    if NameTypeSupport::query().namespace_supported() {
        test_stream(true)?;
    }
    Ok(())
}

#[test]
fn no_server_file() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(false)?;
    }
    Ok(())
}
#[test]
fn no_server_namespaced() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(true)?;
    }
    Ok(())
}
