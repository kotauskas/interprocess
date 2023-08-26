#![cfg(all(windows, feature = "tokio"))]
#[path = "../util/mod.rs"]
#[macro_use]
mod util;

mod bytes;
mod msg;

use color_eyre::eyre::Context;
use interprocess::os::windows::named_pipe::PipeListenerOptions;
use std::{convert::TryInto, ffi::OsStr, future::Future, io, sync::Arc};
use tokio::{sync::oneshot::Sender, task};
use util::{install_color_eyre, listen_and_pick_name, tokio::drive_server_and_multiple_clients, NameGen, TestResult};

#[tokio::test]
async fn tokio_named_pipe_bytes() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_duplex, client_duplex).await
}

#[tokio::test]
async fn tokio_named_pipe_bytes_unidir_client_to_server() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_cts, client_cts).await
}
#[tokio::test]
async fn tokio_named_pipe_bytes_unidir_server_to_client() -> TestResult {
    use bytes::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_stc, client_stc).await
}

#[tokio::test]
async fn tokio_named_pipe_msg() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_duplex, client_duplex).await
}

#[tokio::test]
async fn tokio_named_pipe_msg_unidir_client_to_server() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_cts, client_cts).await
}
#[tokio::test]
async fn tokio_named_pipe_msg_unidir_server_to_client() -> TestResult {
    use msg::*;
    install_color_eyre();
    drive_server_and_multiple_clients(server_stc, client_stc).await
}

async fn drive_server<L, T: Future<Output = TestResult> + Send + 'static>(
    name_sender: Sender<Arc<str>>,
    num_clients: u32,
    mut createfn: impl (FnMut(PipeListenerOptions) -> io::Result<L>),
    mut acceptfut: impl FnMut(Arc<L>) -> T,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), true), |nm| {
        createfn(PipeListenerOptions::new().name(nm.as_ref() as &OsStr)).map(Arc::new)
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
