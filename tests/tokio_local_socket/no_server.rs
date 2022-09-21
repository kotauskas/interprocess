//! Tests what happens when a client attempts to connect to a local socket that doesn't exist.

use {super::util::*, anyhow::*, interprocess::local_socket::tokio::LocalSocketStream, std::io};

pub async fn run_and_verify_error(prefer_namespaced: bool) -> TestResult {
    use io::ErrorKind::*;
    let err = match client(prefer_namespaced).await {
        Err(e) => e.downcast::<io::Error>()?,
        Ok(()) => bail!("client successfully connected to nonexistent server"),
    };
    ensure!(
        matches!(err.kind(), NotFound | ConnectionRefused),
        "expected error to be 'not found' or 'connection refused', received '{}'",
        err
    );
    Ok(())
}
async fn client(prefer_namespaced: bool) -> TestResult {
    let name = NameGen::new_auto(prefer_namespaced).next().unwrap();

    LocalSocketStream::connect(name.as_str())
        .await
        .context("Connect failed")?;
    Ok(())
}
