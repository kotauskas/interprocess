use super::{drive_server, util::*};
use color_eyre::eyre::Context;
use interprocess::{
    os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListener, RecvPipeStream, SendPipeStream},
    reliable_recv_msg::*,
};
use std::{
    str,
    sync::{mpsc::Sender, Arc},
};

fn msgs(server: bool) -> [Box<str>; 2] {
    [
        message(Some(format_args!("First")), server, None),
        message(Some(format_args!("Second")), server, None),
    ]
}
fn futf8(m: &[u8]) -> TestResult<&str> {
    str::from_utf8(m).context("received message was not valid UTF-8")
}

fn handle_conn_duplex(listener: &mut PipeListener<pipe_mode::Messages, pipe_mode::Messages>) -> TestResult {
    let (mut recver, mut sender) = listener.accept().context("accept failed")?.split();

    let [msg1, msg2] = msgs(false);
    recv(&mut recver, msg1, 0)?;
    recv(&mut recver, msg2, 1)?;

    let [msg1, msg2] = msgs(true);
    send(&mut sender, msg1, 0)?;
    send(&mut sender, msg2, 1)?;

    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
fn handle_conn_cts(listener: &mut PipeListener<pipe_mode::Messages, pipe_mode::None>) -> TestResult {
    let mut recver = listener.accept().context("accept failed")?;
    let [msg1, msg2] = msgs(false);
    recv(&mut recver, msg1, 0)?;
    recv(&mut recver, msg2, 1)
}
fn handle_conn_stc(listener: &mut PipeListener<pipe_mode::None, pipe_mode::Messages>) -> TestResult {
    let mut sender = listener.accept().context("accept failed")?;
    let [msg1, msg2] = msgs(true);
    send(&mut sender, msg1, 0)?;
    send(&mut sender, msg2, 1)
}

pub fn server_duplex(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_duplex::<pipe_mode::Messages>(),
        handle_conn_duplex,
    )
}
pub fn server_cts(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_recv_only::<pipe_mode::Messages>(),
        handle_conn_cts,
    )
}
pub fn server_stc(name_sender: Sender<Arc<str>>, num_clients: u32) -> TestResult {
    drive_server(
        name_sender,
        num_clients,
        |plo| plo.create_send_only::<pipe_mode::Messages>(),
        handle_conn_stc,
    )
}

pub fn client_duplex(name: &str) -> TestResult {
    let (mut recver, mut sender) = DuplexPipeStream::<pipe_mode::Messages>::connect(name)
        .context("connect failed")?
        .split();

    let [msg1, msg2] = msgs(false);
    send(&mut sender, msg1, 0)?;
    send(&mut sender, msg2, 1)?;

    let [msg1, msg2] = msgs(true);
    recv(&mut recver, msg1, 0)?;
    recv(&mut recver, msg2, 1)?;

    DuplexPipeStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}
pub fn client_cts(name: &str) -> TestResult {
    let mut sender = SendPipeStream::<pipe_mode::Messages>::connect(name).context("connect failed")?;
    let [msg1, msg2] = msgs(false);
    send(&mut sender, msg1, 0)?;
    send(&mut sender, msg2, 1)
}
pub fn client_stc(name: &str) -> TestResult {
    let mut recver = RecvPipeStream::<pipe_mode::Messages>::connect(name).context("connect failed")?;
    let [msg1, msg2] = msgs(true);
    recv(&mut recver, msg1, 0)?;
    recv(&mut recver, msg2, 1)
}

fn recv(conn: &mut RecvPipeStream<pipe_mode::Messages>, exp: impl AsRef<str>, nr: u8) -> TestResult {
    let fs = ["first", "second"][nr as usize];
    let exp_ = exp.as_ref();
    let mut buf = Vec::with_capacity(exp_.len());

    let rslt = conn.recv(&mut buf).with_context(|| format!("{} receive failed", fs))?;

    ensure_eq!(rslt.size(), exp_.len());
    ensure_eq!(futf8(rslt.borrow_to_size(&buf))?, exp_);
    Ok(())
}

fn send(conn: &mut SendPipeStream<pipe_mode::Messages>, msg: impl AsRef<str>, nr: u8) -> TestResult {
    let msg_ = msg.as_ref();
    let fs = ["first", "second"][nr as usize];

    let sent = conn
        .send(msg_.as_bytes())
        .with_context(|| format!("{} send failed", fs))?;

    ensure_eq!(sent, msg_.len());
    Ok(())
}
