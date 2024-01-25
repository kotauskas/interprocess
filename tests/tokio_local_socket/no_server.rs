//! Tests what happens when a client attempts to connect to a local socket that doesn't exist.

use super::util::*;
use color_eyre::eyre::{bail, ensure};
use interprocess::local_socket::tokio::LocalSocketStream;
use std::io;

pub async fn run_and_verify_error(namespaced: bool) -> TestResult {
    use io::ErrorKind::*;
    let err = match client(namespaced).await {
        Err(e) => e,
        Ok(()) => bail!("client successfully connected to nonexistent server"),
    };
    ensure!(
        matches!(err.kind(), NotFound | ConnectionRefused),
        "expected error to be 'not found' or 'connection refused', received '{}'",
        err
    );
    Ok(())
}
async fn client(namespaced: bool) -> io::Result<()> {
    let nm = NameGen::new(make_id!(), namespaced).next().unwrap();
    LocalSocketStream::connect(&*nm).await?;
    Ok(())
}
