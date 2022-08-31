mod util;
use util::*;

use {
    anyhow::Context,
    interprocess::local_socket::{LocalSocketListener, LocalSocketStream, NameTypeSupport},
    std::{
        io::{self, BufRead, BufReader, Write},
        sync::{mpsc::Sender, Arc},
    },
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    let mut rng = Xorshift32::from_system_time();
    let (name, listener) = loop {
        let rn = rng.next();
        let name = {
            use NameTypeSupport::*;
            match NameTypeSupport::query() {
                OnlyPaths => format!("/tmp/interprocess-test-{:08x}.sock", rn),
                OnlyNamespaced | Both => format!("@interprocess-test-{:08x}.sock", rn),
            }
        };

        let listener = match LocalSocketListener::bind(&*name) {
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => continue,
            x => x.context("Listener bind failed")?,
        };
        break (name, listener);
    };

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
fn client(name: Arc<String>) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let conn = LocalSocketStream::connect(name.as_str()).context("Connect failed")?;
    let mut conn = BufReader::new(conn);

    conn.get_mut()
        .write_all(CLIENT_MSG.as_bytes())
        .context("Socket send failed")?;

    conn.read_line(&mut buffer)
        .context("Socket receive failed")?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}

#[test]
fn local_socket_clsrv() {
    drive_server_and_multiple_clients(server, client);
}
