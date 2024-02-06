use crate::{
    local_socket::{
        tokio::{LocalSocketListener, LocalSocketStream},
        LocalSocketName,
    },
    tests::util::*,
};
use ::tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::oneshot::Sender,
    task, try_join,
};
use color_eyre::eyre::WrapErr;
use std::{convert::TryInto, str, sync::Arc};

fn msg(server: bool, nts: bool) -> Box<str> {
    message(None, server, Some(['\n', '\0'][nts as usize]))
}

pub async fn server(
    id: &'static str,
    name_sender: Sender<Arc<LocalSocketName<'static>>>,
    num_clients: u32,
    namespaced: bool,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut namegen_local_socket(id, namespaced), |nm| {
        LocalSocketListener::bind(nm.borrow())
    })?;

    let _ = name_sender.send(name);

    let mut tasks = Vec::with_capacity(num_clients.try_into().unwrap());
    for _ in 0..num_clients {
        let stream = listener.accept().await.opname("accept")?;
        tasks.push(task::spawn(async move {
            try_join!(
                recv(&stream, msg(false, false), msg(false, true)),
                send(&stream, msg(true, false), msg(true, true)),
            )
        }));
    }
    for task in tasks {
        task.await
            .context("server task panicked")?
            .context("server task returned early with error")?;
    }
    Ok(())
}
pub async fn client(nm: Arc<LocalSocketName<'static>>) -> TestResult {
    let stream = LocalSocketStream::connect(nm.borrow())
        .await
        .opname("connect")?;
    try_join!(
        recv(&stream, msg(true, false), msg(true, true)),
        send(&stream, msg(false, false), msg(false, true)),
    )
    .map(|((), ())| ())
}

async fn recv(
    stream: &LocalSocketStream,
    exp1: impl AsRef<str>,
    exp2: impl AsRef<str>,
) -> TestResult {
    let mut recver = BufReader::new(stream);
    let mut sbuffer = String::with_capacity(128);

    recver
        .read_line(&mut sbuffer)
        .await
        .context("first receive failed")?;
    ensure_eq!(sbuffer, exp1.as_ref());
    sbuffer.clear();
    let mut buffer = sbuffer.into_bytes();

    recver
        .read_until(b'\0', &mut buffer)
        .await
        .context("second receive failed")?;
    ensure_eq!(
        str::from_utf8(&buffer).context("second received message was not valid UTF-8")?,
        exp2.as_ref(),
    );

    Ok(())
}
async fn send(
    mut stream: &LocalSocketStream,
    msg1: impl AsRef<str>,
    msg2: impl AsRef<str>,
) -> TestResult {
    stream
        .write_all(msg1.as_ref().as_bytes())
        .await
        .context("first send failed")?;
    stream
        .write_all(msg2.as_ref().as_bytes())
        .await
        .context("second send failed")?;
    Ok(())
}
