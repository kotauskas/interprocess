use super::util::*;
use color_eyre::eyre::Context;
use interprocess::os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListenerOptions};
use std::{
    ffi::OsStr,
    io::{prelude::*, BufReader},
    sync::mpsc::Sender,
};

pub fn server(name_sender: Sender<String>, num_clients: u32, recv: bool, send: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), true), |nm| {
        PipeListenerOptions::new()
            .name(nm.as_ref() as &OsStr)
            .create_duplex::<pipe_mode::Bytes>()
    })?;

    let _ = name_sender.send(name);

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = listener.accept().context("accept failed").map(BufReader::new)?;

        if recv {
            let expected = message(false, Some('\n'));
            conn.read_line(&mut buffer).context("pipe receive failed")?;
            ensure_eq!(buffer, expected);
            buffer.clear();
        }

        if send {
            let msg = message(true, Some('\n'));
            conn.get_mut().write_all(msg.as_bytes()).context("pipe send failed")?;
            conn.get_mut().flush().context("pipe flush failed")?;
        }
    }

    Ok(())
}
pub fn client(name: &str, recv: bool, send: bool) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut conn = DuplexPipeStream::<pipe_mode::Bytes>::connect(name)
        .context("connect failed")
        .map(BufReader::new)?;

    if send {
        let msg = message(false, Some('\n'));
        conn.get_mut().write_all(msg.as_bytes()).context("pipe send failed")?;
    }

    if recv {
        let expected = message(true, Some('\n'));
        conn.read_line(&mut buffer).context("pipe receive failed")?;
        ensure_eq!(buffer, expected);
    }

    if send {
        // FIXME
        conn.get_mut().flush().context("pipe flush failed")?;
    }

    Ok(())
}
