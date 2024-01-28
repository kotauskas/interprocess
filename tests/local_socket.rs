// TODO test various error conditions

mod no_server;
mod stream;

use crate::{local_socket::NameTypeSupport, tests::util::*};

fn test_stream(split: bool, nmspc: bool) -> TestResult {
    use stream::*;
    testinit();
    let hcl = if split {
        handle_client_split as _
    } else {
        handle_client_nosplit as _
    };
    let scl = |s, n| server(hcl, s, n, nmspc);
    // I love the Rust typesystem
    if split {
        drive_server_and_multiple_clients(scl, client_split)?;
    } else {
        drive_server_and_multiple_clients(scl, client_nosplit)?;
    }
    Ok(())
}

macro_rules! matrix {
    (@querymethod true $e:expr) => { NameTypeSupport::namespace_supported($e) };
    (@querymethod false $e:expr) => { NameTypeSupport::paths_supported($e) };
    (@body $split:ident $nmspc:ident) => {{
        if matrix!(@querymethod $nmspc NameTypeSupport::query()) {
            test_stream(true, $nmspc)?;
        }
        Ok(())
    }};
    ($nm:ident false $nmspc:ident) => {
        #[test]
        fn $nm() -> TestResult { matrix!(@body false $nmspc) }
    };
    ($nm:ident true $nmspc:ident) => {
        #[test]
        #[cfg(not(windows))]
        fn $nm() -> TestResult { matrix!(@body true $nmspc) }
    };
    ($($nm:ident $split:ident $nmspc:ident)+) => { $(matrix!($nm $split $nmspc);)+ };
}

matrix! {
    stream_file_nosplit       false false
    stream_file_split          true false
    stream_namespaced_nosplit false  true
    stream_namespaced_split    true  true
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
