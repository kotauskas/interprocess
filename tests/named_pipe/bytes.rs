use super::drive_server;
use crate::{
    os::windows::named_pipe::{
        pipe_mode, DuplexPipeStream, PipeListener, RecvPipeStream, SendPipeStream,
    },
    tests::util::*,
};
use color_eyre::eyre::Context;
use std::{
    io::{prelude::*, BufReader},
    sync::{mpsc::Sender, Arc},
};

fn msg(server: bool) -> Box<str> {
    message(None, server, Some('\n'))
}

fn handle_conn_duplex(
    listener: &mut PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>,
) -> TestResult {
    let (mut recver, mut sender) = listener.accept().context("accept failed")?.split();
    recv(&mut recver, msg(false))?;
    send(&mut sender, msg(true))?;
    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
fn handle_conn_cts(listener: &mut PipeListener<pipe_mode::Bytes, pipe_mode::None>) -> TestResult {
    let mut recver = listener.accept().context("accept failed")?;
    recv(&mut recver, msg(false))
}
fn handle_conn_stc(listener: &mut PipeListener<pipe_mode::None, pipe_mode::Bytes>) -> TestResult {
    let mut sender = listener.accept().context("accept failed")?;
    send(&mut sender, msg(true))
}

pub fn server_duplex(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        make_id!(),
        name_sender,
        num_clients,
        |plo| plo.create_duplex::<pipe_mode::Bytes>(),
        handle_conn_duplex,
    )
}
pub fn server_cts(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        make_id!(),
        name_sender,
        num_clients,
        |plo| plo.create_recv_only::<pipe_mode::Bytes>(),
        handle_conn_cts,
    )
}
pub fn server_stc(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        make_id!(),
        name_sender,
        num_clients,
        |plo| plo.create_send_only::<pipe_mode::Bytes>(),
        handle_conn_stc,
    )
}

pub fn client_duplex(name: &str) -> TestResult {
    let (mut recver, mut sender) =
        DuplexPipeStream::<pipe_mode::Bytes>::connect(name).context("connect failed")?.split();
    send(&mut sender, msg(false))?;
    recv(&mut recver, msg(true))?;
    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
pub fn client_cts(name: &str) -> TestResult {
    let mut sender = SendPipeStream::<pipe_mode::Bytes>::connect(name).context("connect failed")?;
    send(&mut sender, msg(false))
}
pub fn client_stc(name: &str) -> TestResult {
    let mut recver = RecvPipeStream::<pipe_mode::Bytes>::connect(name).context("connect failed")?;
    recv(&mut recver, msg(true))
}

fn recv(conn: &mut RecvPipeStream<pipe_mode::Bytes>, exp: impl AsRef<str>) -> TestResult {
    let mut conn = BufReader::new(conn);
    let exp_ = exp.as_ref();
    let mut buf = String::with_capacity(exp_.len());
    conn.read_line(&mut buf).context("receive failed")?;
    ensure_eq!(buf, exp_);
    Ok(())
}
fn send(conn: &mut SendPipeStream<pipe_mode::Bytes>, msg: impl AsRef<str>) -> TestResult {
    conn.write_all(msg.as_ref().as_bytes()).context("send failed")?;
    conn.flush().context("flush failed")
}
