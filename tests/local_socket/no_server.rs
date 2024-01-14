//! Tests what happens when a client attempts to connect to a local socket that doesn't exist.

use super::util::*;
use color_eyre::eyre::{bail, ensure};
use interprocess::local_socket::LocalSocketStream;
use std::io;

pub fn run_and_verify_error(prefer_namespaced: bool) -> TestResult {
    use io::ErrorKind::*;
    let err = match client(prefer_namespaced) {
        Err(e) => e,
        Ok(()) => bail!("client successfully connected to nonexistent server"),
    };
    ensure!(
        matches!(err.kind(), NotFound | ConnectionRefused),
        "expected error to be 'not found', received '{}'",
        err
    );
    Ok(())
}
fn client(prefer_namespaced: bool) -> io::Result<()> {
    let name = NameGen::new_auto(make_id!(), prefer_namespaced).next().unwrap();
    LocalSocketStream::connect(&*name)?;
    Ok(())
}
