// TODO(2.3.1) test various error conditions

mod no_client;
mod no_server;
mod stream;

use {
    crate::{local_socket::prelude::*, tests::util::*},
    color_eyre::eyre::bail,
    std::io,
};

fn test_stream(id: &'static str, path: bool) -> TestResult {
    use {io::ErrorKind::*, stream::*};
    let scl = |s, n| server(id, handle_client, s, n, path);
    let name = drive_server_and_multiple_clients(scl, client)?;
    match LocalSocketStream::connect(name.borrow()) {
        Err(e) if matches!(e.kind(), NotFound | ConnectionRefused) => Ok(()),
        Err(e) => bail!(
            "expected NotFound or ConnectionRefused when connecting to dropped listener, got {e}"
        ),
        Ok(s) => bail!("unexpectedly succeeded in connecting to dropped listener: {s:?}"),
    }
}

use {
    no_client::run_and_verify_error as test_no_client,
    no_server::run_and_verify_error as test_no_server,
};

macro_rules! tests {
    ($fn:ident $nm:ident $path:ident) => {
        #[test]
        fn $nm() -> TestResult {
            test_wrapper(|| { $fn(make_id!(), $path) })
        }
    };
    ($fn:ident $($nm:ident $path:ident)+) => { $(tests!($fn $nm $path);)+ };
}

tests! {test_stream
    stream_file       true
    stream_namespaced false
}

tests! {test_no_server
    no_server_file       true
    no_server_namespaced false
}

tests! {test_no_client
    no_client_file       true
    no_client_namespaced false
}
