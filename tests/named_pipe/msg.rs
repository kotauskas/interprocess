use super::util::*;
use color_eyre::eyre::Context;
use interprocess::{
    os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListenerOptions, PipeMode},
    reliable_recv_msg::*,
};
use std::{ffi::OsStr, sync::mpsc::Sender};

const SERVER_MSG_1: &[u8] = b"First server message";
const SERVER_MSG_2: &[u8] = b"Second server message";

const CLIENT_MSG_1: &[u8] = b"First client message";
const CLIENT_MSG_2: &[u8] = b"Second client message";

pub fn server(name_sender: Sender<String>, num_clients: u32, recv: bool, send: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), true), |nm| {
        PipeListenerOptions::new()
            .name(nm.as_ref() as &OsStr)
            .mode(PipeMode::Messages)
            .create_duplex::<pipe_mode::Messages>()
    })?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        let mut conn = listener.accept().context("incoming connection failed")?;

        if recv {
            let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

            let rslt = conn.recv(&mut buf1).context("first pipe receive failed")?;
            ensure_eq!(rslt.size(), CLIENT_MSG_1.len());
            ensure_eq!(rslt.borrow_to_size(&buf1), CLIENT_MSG_1);

            let rslt = conn.recv(&mut buf2).context("second pipe receive failed")?;
            ensure_eq!(rslt.size(), CLIENT_MSG_2.len());
            ensure_eq!(rslt.borrow_to_size(&buf2), CLIENT_MSG_2);
        }

        if send {
            let written = conn.send(SERVER_MSG_1).context("first pipe send failed")?;
            ensure_eq!(written, SERVER_MSG_1.len());

            let written = conn.send(SERVER_MSG_2).context("second pipe send failed")?;
            ensure_eq!(written, SERVER_MSG_2.len());

            conn.flush().context("flush failed")?;
        }
    }

    Ok(())
}
pub fn client(name: &str, recv: bool, send: bool) -> TestResult {
    let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

    let mut conn = DuplexPipeStream::<pipe_mode::Messages>::connect(name).context("connect failed")?;

    if send {
        let written = conn.send(CLIENT_MSG_1).context("first pipe send failed")?;
        ensure_eq!(written, CLIENT_MSG_1.len());

        let written = conn.send(CLIENT_MSG_2).context("second pipe send failed")?;
        ensure_eq!(written, CLIENT_MSG_2.len());

        conn.flush().context("flush failed")?;
    }

    if recv {
        let rslt = conn.recv(&mut buf1).context("first pipe receive failed")?;
        ensure_eq!(rslt.size(), SERVER_MSG_1.len());
        ensure_eq!(rslt.borrow_to_size(&buf1), SERVER_MSG_1);

        let rslt = conn.recv(&mut buf2).context("second pipe receive failed")?;
        ensure_eq!(rslt.size(), SERVER_MSG_2.len());
        ensure_eq!(rslt.borrow_to_size(&buf2), SERVER_MSG_2);
    }

    Ok(())
}
