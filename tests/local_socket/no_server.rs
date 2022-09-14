//! Tests what happens when a client attempts to connect to a local socket that doesn't exist.

use {super::util::*, anyhow::*, interprocess::local_socket::LocalSocketStream, std::io};

pub fn run_and_verify_error(prefer_namespaced: bool) -> TestResult {
    use io::ErrorKind::*;
    let err = match client(prefer_namespaced) {
        Err(e) => e.downcast::<io::Error>()?,
        Ok(()) => bail!("client successfully connected to nonexistent server"),
    };
    ensure!(
        matches!(err.kind(), NotFound | ConnectionRefused),
        "expected error to be 'not found', received '{}'",
        err
    );
    Ok(())
}
fn client(prefer_namespaced: bool) -> TestResult {
    let name = NameGen::new_auto(prefer_namespaced).next().unwrap();

    LocalSocketStream::connect(name.as_str()).context("Connect failed")?;
    Ok(())
}
