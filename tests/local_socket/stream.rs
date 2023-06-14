use {
    super::{util::*, NameGen},
    anyhow::Context,
    interprocess::local_socket::{LocalSocketListener, LocalSocketStream},
    std::{
        io::{self, BufRead, BufReader, Write},
        sync::{mpsc::Sender, Arc},
    },
};

static SERVER_LINE: &[u8] = b"Hello from server!\n";
static SERVER_BYTES: &[u8] = b"Bytes from server!\0";
static CLIENT_LINE: &[u8] = b"Hello from client!\n";
static CLIENT_BYTES: &[u8] = b"Bytes from client!\0";

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

        conn.read_until(b'\n', &mut buffer)
            .context("First socket receive failed")?;
        assert_eq!(buffer, CLIENT_LINE);
        buffer.clear();

        conn.get_mut()
            .write_all(SERVER_LINE)
            .context("First socket send failed")?;

        conn.read_until(b'\0', &mut buffer)
            .context("Second socket receive failed")?;
        assert_eq!(buffer, CLIENT_BYTES);
        buffer.clear();

        conn.get_mut()
            .write_all(SERVER_BYTES)
            .context("Second socket send failed")?;
    }
    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let mut buffer = Vec::with_capacity(128);

    let mut conn = LocalSocketStream::connect(name.as_str())
        .context("Connect failed")
        .map(BufReader::new)?;

    conn.get_mut()
        .write_all(CLIENT_LINE)
        .context("First socket send failed")?;

    conn.read_until(b'\n', &mut buffer)
        .context("First socket receive failed")?;
    assert_eq!(buffer, SERVER_LINE);
    buffer.clear();

    conn.get_mut()
        .write_all(CLIENT_BYTES)
        .context("Second socket send failed")?;

    conn.read_until(b'\0', &mut buffer)
        .context("Second socket receive failed")?;
    assert_eq!(buffer, SERVER_BYTES);

    Ok(())
}
