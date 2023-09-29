use super::{drive_server, util::*};
use color_eyre::eyre::Context;
use interprocess::os::windows::named_pipe::{
    pipe_mode, DuplexPipeStream, PipeListener, RecvPipeStream, SendPipeStream,
};
use std::{
    io::{prelude::*, BufReader},
    sync::{mpsc::Sender, Arc},
};

// TODO reunite testing

fn msg(server: bool) -> Box<str> {
    message(None, server, None)
}

fn handle_conn_duplex(listener: &mut PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>) -> TestResult {
    let (recver, sender) = listener.accept().context("accept failed")?.split();
    recv(recver, msg(false))?;
    send(sender, msg(true))
}
fn handle_conn_cts(listener: &mut PipeListener<pipe_mode::Bytes, pipe_mode::None>) -> TestResult {
    let recver = listener.accept().context("accept failed")?;
    recv(recver, msg(false))
}
fn handle_conn_stc(listener: &mut PipeListener<pipe_mode::None, pipe_mode::Bytes>) -> TestResult {
    let sender = listener.accept().context("accept failed")?;
    send(sender, msg(true))
}

pub fn server_duplex(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_duplex::<pipe_mode::Bytes>(),
        handle_conn_duplex,
    )
}
pub fn server_cts(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_recv_only::<pipe_mode::Bytes>(),
        handle_conn_cts,
    )
}
pub fn server_stc(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_send_only::<pipe_mode::Bytes>(),
        handle_conn_stc,
    )
}

pub fn client_duplex(name: &str) -> TestResult {
    let (recver, sender) = DuplexPipeStream::<pipe_mode::Bytes>::connect(name)
        .context("connect failed")?
        .split();
    send(sender, msg(false))?;
    recv(recver, msg(true))
}
pub fn client_cts(name: &str) -> TestResult {
    let sender = SendPipeStream::<pipe_mode::Bytes>::connect(name).context("connect failed")?;
    send(sender, msg(false))
}
pub fn client_stc(name: &str) -> TestResult {
    let recver = RecvPipeStream::<pipe_mode::Bytes>::connect(name).context("connect failed")?;
    recv(recver, msg(true))
}

fn recv(conn: RecvPipeStream<pipe_mode::Bytes>, exp: impl AsRef<str>) -> TestResult {
    let mut conn = BufReader::new(conn);
    let exp_ = exp.as_ref();
    let mut buf = String::with_capacity(exp_.len());
    conn.read_line(&mut buf).context("receive failed")?;
    ensure_eq!(buf, exp_);
    Ok(())
}
fn send(mut conn: SendPipeStream<pipe_mode::Bytes>, msg: impl AsRef<str>) -> TestResult {
    conn.write_all(msg.as_ref().as_bytes()).context("send failed")?;
    conn.flush().context("flush failed")
}
