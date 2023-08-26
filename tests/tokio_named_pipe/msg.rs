use super::{drive_server, util::TestResult};
use color_eyre::eyre::Context;
use interprocess::{
    os::windows::named_pipe::{
        pipe_mode,
        tokio::{self as np, DuplexPipeStream, PipeListener, PipeListenerOptionsExt, RecvPipeStream, SendPipeStream},
    },
    reliable_recv_msg::AsyncReliableRecvMsgExt,
};
use std::sync::Arc;
use tokio::{sync::oneshot::Sender, try_join};

const SERVER_MSG_1: &[u8] = b"First server message";
const SERVER_MSG_2: &[u8] = b"Second server message";

const CLIENT_MSG_1: &[u8] = b"First client message";
const CLIENT_MSG_2: &[u8] = b"Second client message";

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
    let (recver, sender) = listener.accept().await.context("accept failed")?.split();
    try_join!(
        recv(recver, CLIENT_MSG_1, CLIENT_MSG_2),
        send(sender, SERVER_MSG_1, SERVER_MSG_2),
    )
    .map(|((), ())| ())
}
async fn handle_conn_cts(listener: Arc<PipeListener<pipe_mode::Messages, pipe_mode::None>>) -> TestResult {
    let recver = listener.accept().await.context("accept failed")?.into_recv_half();
    recv(recver, CLIENT_MSG_1, CLIENT_MSG_2).await
}
async fn handle_conn_stc(listener: Arc<PipeListener<pipe_mode::None, pipe_mode::Messages>>) -> TestResult {
    let sender = listener.accept().await.context("accept failed")?.into_send_half();
    send(sender, SERVER_MSG_1, SERVER_MSG_2).await
}

pub async fn client_duplex(nm: Arc<str>) -> TestResult {
    let (recver, sender) = DuplexPipeStream::<pipe_mode::Messages>::connect(&*nm)
        .await
        .context("connect failed")?
        .split();

    try_join!(
        recv(recver, SERVER_MSG_1, SERVER_MSG_2),
        send(sender, CLIENT_MSG_1, CLIENT_MSG_2),
    )
    .map(|((), ())| ())
}
pub async fn client_cts(name: Arc<str>) -> TestResult {
    let sender = SendPipeStream::<pipe_mode::Messages>::connect(&*name)
        .await
        .context("connect failed")?
        .into_send_half();

    send(sender, CLIENT_MSG_1, CLIENT_MSG_2).await
}
pub async fn client_stc(name: Arc<str>) -> TestResult {
    let recver = RecvPipeStream::<pipe_mode::Messages>::connect(&*name)
        .await
        .context("connect failed")?
        .into_recv_half();

    recv(recver, SERVER_MSG_1, SERVER_MSG_2).await
}

async fn recv(recver: np::RecvHalf<pipe_mode::Messages>, exp1: &[u8], exp2: &[u8]) -> TestResult {
    let mut buf = Vec::with_capacity(exp1.len());

    let rslt = (&recver).recv(&mut buf).await.context("first receive failed")?;
    ensure_eq!(rslt.size(), exp1.len());
    ensure_eq!(rslt.borrow_to_size(&buf), exp1);

    buf.clear();
    buf.reserve(exp2.len().saturating_sub(exp1.len()));
    let rslt = (&recver).recv(&mut buf).await.context("second receive failed")?;
    ensure_eq!(rslt.size(), exp2.len());
    ensure_eq!(rslt.borrow_to_size(&buf), exp2);

    Ok(())
}
async fn send(sender: np::SendHalf<pipe_mode::Messages>, snd1: &[u8], snd2: &[u8]) -> TestResult {
    let sent = sender.send(snd1).await.context("first send failed")?;
    ensure_eq!(sent, snd1.len());

    let sent = sender.send(snd2).await.context("second send failed")?;
    ensure_eq!(sent, snd2.len());

    sender.flush().await.context("flush failed")?;

    Ok(())
}
