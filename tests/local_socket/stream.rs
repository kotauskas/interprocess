use {
    super::{util::*, NameGen},
    anyhow::Context,
    interprocess::local_socket::{LocalSocketListener, LocalSocketStream},
    std::{
        io::{self, BufRead, BufReader, Write},
        sync::{mpsc::Sender, Arc},
    },
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub fn server(
    name_sender: Sender<String>,
    num_clients: u32,
    prefer_namespaced: bool,
) -> TestResult {
    let (name, listener) = NameGen::new_auto(prefer_namespaced)
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

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {}", e);
                continue;
            }
        };

        conn.read_line(&mut buffer)
            .context("Socket receive failed")?;

        conn.get_mut()
            .write_all(SERVER_MSG.as_bytes())
            .context("Socket send failed")?;

        assert_eq!(buffer, CLIENT_MSG);
        buffer.clear();
    }
    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut conn = LocalSocketStream::connect(name.as_str())
        .context("Connect failed")
        .map(BufReader::new)?;

    conn.get_mut()
        .write_all(CLIENT_MSG.as_bytes())
        .context("Socket send failed")?;

    conn.read_line(&mut buffer)
        .context("Socket receive failed")?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
