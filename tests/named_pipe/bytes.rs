use super::util::*;
use color_eyre::eyre::{bail, Context};
use interprocess::os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListenerOptions};
use std::{
    ffi::OsStr,
    io::{self, prelude::*, BufReader},
    sync::mpsc::Sender,
};
// TODO context instead of bail, ensure_eq, use listen_and_pick_name

pub fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    let (name, listener) = NameGen::new(make_id!(), true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new().name(rnm).create_duplex::<pipe_mode::Bytes>() {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("listener bind failed")?;

    let _ = name_sender.send(name);

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => bail!("incoming connection failed: {e}"),
        };

        let expected = message(false, Some('\n'));
        conn.read_line(&mut buffer).context("pipe receive failed")?;
        assert_eq!(buffer, expected);
        buffer.clear();

        let msg = message(true, Some('\n'));
        conn.get_mut().write_all(msg.as_bytes()).context("pipe send failed")?;
        conn.get_mut().flush().context("pipe flush failed")?;
    }

    Ok(())
}
pub fn client(name: &str) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut conn = DuplexPipeStream::<pipe_mode::Bytes>::connect(name)
        .context("connect failed")
        .map(BufReader::new)?;

    let msg = message(false, Some('\n'));
    conn.get_mut().write_all(msg.as_bytes()).context("pipe send failed")?;

    let expected = message(true, Some('\n'));
    conn.read_line(&mut buffer).context("pipe receive failed")?;
    assert_eq!(buffer, expected);

    conn.get_mut().flush().context("pipe flush failed")?;

    Ok(())
}
