use super::{
    drive_server,
    util::{message, TestResult},
};
use color_eyre::eyre::Context;
use futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use interprocess::os::windows::named_pipe::{
    pipe_mode,
    tokio::{DuplexPipeStream, PipeListener, PipeListenerOptionsExt, RecvPipeStream, SendPipeStream},
};
use std::sync::Arc;
use tokio::{sync::oneshot::Sender, try_join};

fn msg(server: bool) -> Box<str> {
    message(None, server, Some('\n'))
}

pub async fn server_duplex(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_duplex::<pipe_mode::Bytes>(),
        handle_conn_duplex,
    )
    .await
}
pub async fn server_cts(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_recv_only::<pipe_mode::Bytes>(),
        handle_conn_cts,
    )
    .await
}
pub async fn server_stc(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_tokio_send_only::<pipe_mode::Bytes>(),
        handle_conn_stc,
    )
    .await
}

async fn handle_conn_duplex(listener: Arc<PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>>) -> TestResult {
    let conn = listener.accept().await.context("accept failed")?;
    let (reader, writer) = conn.split();
    try_join!(read(reader, msg(false)), write(writer, msg(true))).map(|((), ())| ())
}
async fn handle_conn_cts(listener: Arc<PipeListener<pipe_mode::Bytes, pipe_mode::None>>) -> TestResult {
    let conn = listener.accept().await.context("accept failed")?;
    read(conn, msg(false)).await
}
async fn handle_conn_stc(listener: Arc<PipeListener<pipe_mode::None, pipe_mode::Bytes>>) -> TestResult {
    let conn = listener.accept().await.context("accept failed")?;
    write(conn, msg(true)).await
}

pub async fn client_duplex(name: Arc<str>) -> TestResult {
    let (reader, writer) = DuplexPipeStream::<pipe_mode::Bytes>::connect(&*name)
        .await
        .context("connect failed")?
        .split();
    try_join!(read(reader, msg(true)), write(writer, msg(false))).map(|((), ())| ())
}
pub async fn client_cts(name: Arc<str>) -> TestResult {
    let writer = SendPipeStream::<pipe_mode::Bytes>::connect(&*name)
        .await
        .context("connect failed")?;
    write(writer, msg(false)).await
}
pub async fn client_stc(name: Arc<str>) -> TestResult {
    let reader = RecvPipeStream::<pipe_mode::Bytes>::connect(&*name)
        .await
        .context("connect failed")?;
    read(reader, msg(true)).await
}

async fn read(reader: RecvPipeStream<pipe_mode::Bytes>, exp: impl AsRef<str>) -> TestResult {
    let mut buffer = String::with_capacity(128);
    let mut reader = BufReader::new(reader);
    reader.read_line(&mut buffer).await.context("pipe receive failed")?;
    ensure_eq!(buffer, exp.as_ref());
    Ok(())
}
async fn write(mut writer: SendPipeStream<pipe_mode::Bytes>, snd: impl AsRef<str>) -> TestResult {
    writer
        .write_all(snd.as_ref().as_bytes())
        .await
        .context("pipe send failed")
}
