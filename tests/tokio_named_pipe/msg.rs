use super::{
    drive_server,
    util::{message, TestResult},
};
use color_eyre::eyre::Context;
use interprocess::{
    os::windows::named_pipe::{
        pipe_mode,
        tokio::{DuplexPipeStream, PipeListener, PipeListenerOptionsExt, RecvPipeStream, SendPipeStream},
    },
    reliable_recv_msg::AsyncReliableRecvMsgExt,
};
use std::{str, sync::Arc};
use tokio::{sync::oneshot::Sender, try_join};

fn msgs(server: bool) -> [Box<str>; 2] {
    [
        message(Some(format_args!("First")), server, None),
        message(Some(format_args!("Second")), server, None),
    ]
}
fn futf8(m: &[u8]) -> TestResult<&str> {
    str::from_utf8(m).context("received message was not valid UTF-8")
}

pub async fn server_duplex(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_duplex::<pipe_mode::Messages>(),
        handle_conn_duplex,
    )
    .await
}
pub async fn server_cts(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_recv_only::<pipe_mode::Messages>(),
        handle_conn_cts,
    )
    .await
}
pub async fn server_stc(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_send_only::<pipe_mode::Messages>(),
        handle_conn_stc,
    )
    .await
}

async fn handle_conn_duplex(listener: Arc<PipeListener<pipe_mode::Messages, pipe_mode::Messages>>) -> TestResult {
    let (mut recver, mut sender) = listener.accept().await.context("accept failed")?.split();
    let [rmsg1, rmsg2] = msgs(false);
    let [smsg1, smsg2] = msgs(true);
    try_join!(recv(&mut recver, &rmsg1, &rmsg2), send(&mut sender, &smsg1, &smsg2))?;
    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
async fn handle_conn_cts(listener: Arc<PipeListener<pipe_mode::Messages, pipe_mode::None>>) -> TestResult {
    let mut recver = listener.accept().await.context("accept failed")?;
    let [rmsg1, rmsg2] = msgs(false);
    recv(&mut recver, &rmsg1, &rmsg2).await
}
async fn handle_conn_stc(listener: Arc<PipeListener<pipe_mode::None, pipe_mode::Messages>>) -> TestResult {
    let mut sender = listener.accept().await.context("accept failed")?;
    let [smsg1, smsg2] = msgs(true);
    send(&mut sender, &smsg1, &smsg2).await
}

pub async fn client_duplex(nm: Arc<str>) -> TestResult {
    let (mut recver, mut sender) = DuplexPipeStream::<pipe_mode::Messages>::connect(&*nm)
        .await
        .context("connect failed")?
        .split();
    let [rmsg1, rmsg2] = msgs(true);
    let [smsg1, smsg2] = msgs(false);
    try_join!(recv(&mut recver, &rmsg1, &rmsg2), send(&mut sender, &smsg1, &smsg2))?;
    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
pub async fn client_cts(name: Arc<str>) -> TestResult {
    let mut sender = SendPipeStream::<pipe_mode::Messages>::connect(&*name)
        .await
        .context("connect failed")?;
    let [smsg1, smsg2] = msgs(false);
    send(&mut sender, &smsg1, &smsg2).await
}
pub async fn client_stc(name: Arc<str>) -> TestResult {
    let mut recver = RecvPipeStream::<pipe_mode::Messages>::connect(&*name)
        .await
        .context("connect failed")?;
    let [rmsg1, rmsg2] = msgs(true);
    recv(&mut recver, &rmsg1, &rmsg2).await
}

async fn recv(recver: &mut RecvPipeStream<pipe_mode::Messages>, exp1: &str, exp2: &str) -> TestResult {
    let mut buf = Vec::with_capacity(exp1.len());

    let rslt = (&*recver).recv(&mut buf).await.context("first receive failed")?;
    ensure_eq!(rslt.size(), exp1.len());
    ensure_eq!(futf8(rslt.borrow_to_size(&buf))?, exp1);

    buf.clear();
    buf.reserve(exp2.len().saturating_sub(exp1.len()));
    let rslt = (&*recver).recv(&mut buf).await.context("second receive failed")?;
    ensure_eq!(rslt.size(), exp2.len());
    ensure_eq!(futf8(rslt.borrow_to_size(&buf))?, exp2);

    Ok(())
}
async fn send(sender: &mut SendPipeStream<pipe_mode::Messages>, snd1: &str, snd2: &str) -> TestResult {
    let sent = sender.send(snd1.as_bytes()).await.context("first send failed")?;
    ensure_eq!(sent, snd1.len());

    let sent = sender.send(snd2.as_bytes()).await.context("second send failed")?;
    ensure_eq!(sent, snd2.len());

    sender.flush().await.context("flush failed")
}
