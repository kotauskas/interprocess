use {
    super::util::*,
    anyhow::Context,
    interprocess::os::unix::udsocket::{UdStream, UdStreamListener},
    std::{
        io,
        sync::{mpsc::Sender, Arc},
    },
};

const SERVER_MSG_1: &[u8] = b"Server message 1";
const SERVER_MSG_2: &[u8] = b"Server message 2";

const CLIENT_MSG_1: &[u8] = b"Client message 1";
const CLIENT_MSG_2: &[u8] = b"Client message 2";

pub(super) fn run_with_namegen(namegen: NameGen) {
    drive_server_and_multiple_clients(move |snd, nc| server(snd, nc, namegen), client);
}

fn server(name_sender: Sender<String>, num_clients: u32, mut namegen: NameGen) -> TestResult {
    let (name, listener) = namegen
        .find_map(|nm| {
            let l = match UdStreamListener::bind(&*nm) {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("Listener bind failed")?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        let conn = match listener.accept() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Incoming connection failed: {}", e);
                continue;
            }
        };

        let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

        let read = conn
            .recv(&mut buf1)
            .context("First socket receive failed")?;
        assert_eq!(read, CLIENT_MSG_1.len());
        assert_eq!(&buf1[0..read], CLIENT_MSG_1);

        let read = conn
            .recv(&mut buf2)
            .context("Second socket receive failed")?;
        assert_eq!(read, CLIENT_MSG_2.len());
        assert_eq!(&buf2[0..read], CLIENT_MSG_2);

        let written = conn
            .send(SERVER_MSG_1)
            .context("First socket send failed")?;
        assert_eq!(written, SERVER_MSG_1.len());

        let written = conn
            .send(SERVER_MSG_2)
            .context("Second socket send failed")?;
        assert_eq!(written, SERVER_MSG_2.len());
    }
    Ok(())
}

fn client(name: Arc<String>) -> TestResult {
    let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);
    let conn = UdStream::connect(name.as_str()).context("Connect failed")?;

    let written = conn
        .send(CLIENT_MSG_1)
        .context("First socket send failed")?;
    assert_eq!(written, CLIENT_MSG_1.len());

    let written = conn
        .send(CLIENT_MSG_2)
        .context("Second socket send failed")?;
    assert_eq!(written, CLIENT_MSG_2.len());

    let read = conn
        .recv(&mut buf1)
        .context("First socket receive failed")?;
    assert_eq!(read, SERVER_MSG_1.len());
    assert_eq!(&buf1[0..read], SERVER_MSG_1);

    let read = conn
        .recv(&mut buf2)
        .context("Second socket receive failed")?;
    assert_eq!(read, SERVER_MSG_2.len());
    assert_eq!(&buf2[0..read], SERVER_MSG_2);

    Ok(())
}
