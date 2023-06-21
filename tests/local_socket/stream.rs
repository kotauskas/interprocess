use super::{util::*, NameGen};
use color_eyre::eyre::Context;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{
    io::{self, BufRead, BufReader, Write},
    str,
    sync::{mpsc::Sender, Arc},
};

pub fn server(name_sender: Sender<String>, num_clients: u32, prefer_namespaced: bool) -> TestResult {
    let (name, listener) = NameGen::new_auto(make_id!(), prefer_namespaced)
        .find_map(|nm| {
            let l = match LocalSocketListener::bind(&*nm) {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("Listener bind failed")?;

    let _ = name_sender.send(name);

    let mut buffer = Vec::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };

        let expected = message(false, Some('\n'));
        conn.read_until(b'\n', &mut buffer)
            .context("First socket receive failed")?;
        assert_eq!(
            str::from_utf8(&buffer).context("First socket receive wasn't valid UTF-8")?,
            expected
        );
        buffer.clear();

        let msg = message(true, Some('\n'));
        conn.get_mut()
            .write_all(msg.as_bytes())
            .context("First socket send failed")?;

        let expected = message(false, Some('\0'));
        conn.read_until(b'\0', &mut buffer)
            .context("Second socket receive failed")?;
        assert_eq!(
            str::from_utf8(&buffer).context("Second socket receive wasn't valid UTF-8")?,
            expected
        );
        buffer.clear();

        let msg = message(true, Some('\0'));
        conn.get_mut()
            .write_all(msg.as_bytes())
            .context("Second socket send failed")?;
    }
    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let mut buffer = Vec::with_capacity(128);

    let mut conn = LocalSocketStream::connect(name.as_str())
        .context("Connect failed")
        .map(BufReader::new)?;

    let msg = message(false, Some('\n'));
    conn.get_mut()
        .write_all(msg.as_bytes())
        .context("First socket send failed")?;

    let expected = message(true, Some('\n'));
    conn.read_until(b'\n', &mut buffer)
        .context("First socket receive failed")?;
    assert_eq!(
        str::from_utf8(&buffer).context("First socket receive wasn't valid UTF-8")?,
        expected
    );
    buffer.clear();

    let msg = message(false, Some('\0'));
    conn.get_mut()
        .write_all(msg.as_bytes())
        .context("Second socket send failed")?;

    let expected = message(true, Some('\0'));
    conn.read_until(b'\0', &mut buffer)
        .context("Second socket receive failed")?;
    assert_eq!(
        str::from_utf8(&buffer).context("Second socket receive wasn't valid UTF-8")?,
        expected
    );

    Ok(())
}
