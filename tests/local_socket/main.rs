#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

mod no_server;
mod stream;

use interprocess::local_socket::NameTypeSupport;

fn local_socket_stream(nmspc: bool) -> TestResult {
    use stream::*;
    testinit();
    util::drive_server_and_multiple_clients(|s, n| server(s, n, nmspc), client)?;
    Ok(())
}

#[test]
fn local_socket_stream_file() -> TestResult {
    if NameTypeSupport::query().paths_supported() {
        local_socket_stream(false)?;
    }
    Ok(())
}
#[test]
fn local_socket_stream_namespaced() -> TestResult {
    if NameTypeSupport::query().namespace_supported() {
        local_socket_stream(true)?;
    }
    Ok(())
}

#[test]
fn local_socket_no_server_file() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(false)?;
    }
    Ok(())
}
#[test]
fn local_socket_no_server_namespaced() -> TestResult {
    testinit();
    if NameTypeSupport::query().paths_supported() {
        no_server::run_and_verify_error(true)?;
    }
    Ok(())
}
