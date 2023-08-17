use super::{util::*, NameGen};
use color_eyre::eyre::{bail, Context};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{
    io::{BufRead, BufReader, Write},
    str,
    sync::mpsc::Sender,
};

pub fn server(name_sender: Sender<String>, num_clients: u32, prefer_namespaced: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new_auto(make_id!(), prefer_namespaced), |nm| {
        LocalSocketListener::bind(nm)
    })?;

    let _ = name_sender.send(name);

    let mut buffer = Vec::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => bail!("incoming connection failed: {e}"),
        };

        let expected = message(false, Some('\n'));
        conn.read_until(b'\n', &mut buffer)
            .context("first socket receive failed")?;
        ensure_eq!(
            str::from_utf8(&buffer).context("first socket receive wasn't valid UTF-8")?,
            expected
        );
        buffer.clear();

        let msg = message(true, Some('\n'));
        conn.get_mut()
            .write_all(msg.as_bytes())
            .context("first socket send failed")?;

        let expected = message(false, Some('\0'));
        conn.read_until(b'\0', &mut buffer)
            .context("second socket receive failed")?;
        ensure_eq!(
            str::from_utf8(&buffer).context("second socket receive wasn't valid UTF-8")?,
            expected
        );
        buffer.clear();

        let msg = message(true, Some('\0'));
        conn.get_mut()
            .write_all(msg.as_bytes())
            .context("second socket send failed")?;
    }
    Ok(())
}
pub fn client(name: &str) -> TestResult {
    let mut buffer = Vec::with_capacity(128);

    let mut conn = LocalSocketStream::connect(name)
        .context("connect failed")
        .map(BufReader::new)?;

    let msg = message(false, Some('\n'));
    conn.get_mut()
        .write_all(msg.as_bytes())
        .context("first socket send failed")?;

    let expected = message(true, Some('\n'));
    conn.read_until(b'\n', &mut buffer)
        .context("first socket receive failed")?;
    ensure_eq!(
        str::from_utf8(&buffer).context("first socket receive wasn't valid UTF-8")?,
        expected
    );
    buffer.clear();

    let msg = message(false, Some('\0'));
    conn.get_mut()
        .write_all(msg.as_bytes())
        .context("second socket send failed")?;

    let expected = message(true, Some('\0'));
    conn.read_until(b'\0', &mut buffer)
        .context("second socket receive failed")?;
    ensure_eq!(
        str::from_utf8(&buffer).context("second socket receive wasn't valid UTF-8")?,
        expected
    );

    Ok(())
}
