#![cfg(all(windows, feature = "tokio"))]

mod bytes;

use crate::{
    os::windows::named_pipe::PipeListenerOptions,
    testutil::{
        listen_and_pick_name, testinit, tokio::drive_server_and_multiple_clients, NameGen,
        TestResult,
    },
};
use color_eyre::eyre::Context;
use std::{convert::TryInto, future::Future, io, path::Path, sync::Arc};
use tokio::{sync::oneshot::Sender, task};

#[tokio::test]
async fn bytes_bidir() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_duplex, client_duplex).await
}

#[tokio::test]
async fn bytes_unidir_client_to_server() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_cts, client_cts).await
}
#[tokio::test]
async fn bytes_unidir_server_to_client() -> TestResult {
    use bytes::*;
    testinit();
    drive_server_and_multiple_clients(server_stc, client_stc).await
}

async fn drive_server<L, T: Future<Output = TestResult> + Send + 'static>(
    id: &'static str,
    name_sender: Sender<Arc<str>>,
    num_clients: u32,
    mut createfn: impl (FnMut(PipeListenerOptions<'_>) -> io::Result<L>),
    mut acceptfut: impl FnMut(Arc<L>) -> T,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(id, false), |nm| {
        createfn(PipeListenerOptions::new().path(Path::new(nm))).map(Arc::new)
    })?;

    let _ = name_sender.send(name);

    let mut tasks = Vec::with_capacity(num_clients.try_into().unwrap());

    for _ in 0..num_clients {
        tasks.push(task::spawn(acceptfut(Arc::clone(&listener))));
    }
    for task in tasks {
        task.await
            .context("server task panicked")?
            .context("server task returned early with error")?;
    }

    Ok(())
}
