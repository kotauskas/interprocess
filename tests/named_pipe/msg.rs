use super::util::*;
use color_eyre::eyre::Context;
use interprocess::{
    os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListenerOptions, PipeMode},
    reliable_recv_msg::*,
};
use std::{
    ffi::OsStr,
    str,
    sync::{mpsc::Sender, Arc},
};

fn msgs(server: bool) -> [Box<str>; 2] {
    [
        message(Some(format_args!("First")), server, None),
        message(Some(format_args!("Second")), server, None),
    ]
}
fn bufs2fit(msg1: &str, msg2: &str) -> [Vec<u8>; 2] {
    [Vec::with_capacity(msg1.len()), Vec::with_capacity(msg2.len())]
}
fn futf8(m: &[u8]) -> TestResult<&str> {
    str::from_utf8(m).context("received message was not valid UTF-8")
}

pub fn server(name_sender: Sender<Arc<str>>, num_clients: u32, recv: bool, send: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), true), |nm| {
        PipeListenerOptions::new()
            .name(nm.as_ref() as &OsStr)
            .mode(PipeMode::Messages)
            .create_duplex::<pipe_mode::Messages>()
    })?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        let mut conn = listener.accept().context("accept failed")?;

        if recv {
            let [msg1, msg2] = msgs(false);
            let [mut buf1, mut buf2] = bufs2fit(&msg1, &msg2);

            let rslt = conn.recv(&mut buf1).context("first pipe receive failed")?;
            ensure_eq!(rslt.size(), msg1.len());
            ensure_eq!(futf8(rslt.borrow_to_size(&buf1))?, &*msg1);

            let rslt = conn.recv(&mut buf2).context("second pipe receive failed")?;
            ensure_eq!(rslt.size(), msg2.len());
            ensure_eq!(futf8(rslt.borrow_to_size(&buf2))?, &*msg2);
        }

        if send {
            let [msg1, msg2] = msgs(true);
            let written = conn.send(msg1.as_bytes()).context("first pipe send failed")?;
            ensure_eq!(written, msg1.len());

            let written = conn.send(msg2.as_bytes()).context("second pipe send failed")?;
            ensure_eq!(written, msg2.len());

            conn.flush().context("flush failed")?;
        }
    }

    Ok(())
}
pub fn client(name: &str, recv: bool, send: bool) -> TestResult {
    let mut conn = DuplexPipeStream::<pipe_mode::Messages>::connect(name).context("connect failed")?;

    if send {
        let [msg1, msg2] = msgs(false);

        let written = conn.send(msg1.as_bytes()).context("first pipe send failed")?;
        ensure_eq!(written, msg2.len());

        let written = conn.send(msg2.as_bytes()).context("second pipe send failed")?;
        ensure_eq!(written, msg2.len());

        conn.flush().context("flush failed")?;
    }

    if recv {
        let [msg1, msg2] = msgs(true);
        let [mut buf1, mut buf2] = bufs2fit(&msg1, &msg2);

        let rslt = conn.recv(&mut buf1).context("first pipe receive failed")?;
        ensure_eq!(rslt.size(), msg1.len());
        ensure_eq!(futf8(rslt.borrow_to_size(&buf1))?, &*msg1);

        let rslt = conn.recv(&mut buf2).context("second pipe receive failed")?;
        ensure_eq!(rslt.size(), msg2.len());
        ensure_eq!(futf8(rslt.borrow_to_size(&buf2))?, &*msg2);
    }

    Ok(())
}
