use {
    super::util::*,
    anyhow::Context,
    interprocess::os::unix::udsocket::{UdStream, UdStreamListener},
    std::{
        io::{self, BufRead, BufReader, Read, Write},
        net::Shutdown,
        sync::{mpsc::Sender, Arc},
    },
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub(super) fn run_with_namegen(namegen: NameGen) {
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, false),
        |nm| client(nm, false),
    );
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, true),
        |nm| client(nm, true),
    );
}

fn server(
    name_sender: Sender<String>,
    num_clients: u32,
    mut namegen: NameGen,
    shutdown: bool,
) -> TestResult {
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

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {}", e);
                continue;
            }
        };

        if shutdown {
            conn.read_to_string(&mut buffer)
        } else {
            conn.read_line(&mut buffer)
        }
        .context("Socket receive failed")?;

        conn.get_mut()
            .write_all(SERVER_MSG.as_bytes())
            .context("Socket send failed")?;
        if shutdown {
            conn.get_mut()
                .shutdown(Shutdown::Write)
                .context("Shutdown of writing end failed")?;
        }

        assert_eq!(buffer, CLIENT_MSG);
        buffer.clear();
    }
    Ok(())
}

fn client(name: Arc<String>, shutdown: bool) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let conn = UdStream::connect(name.as_str()).context("Connect failed")?;
    let mut conn = BufReader::new(conn);

    conn.get_mut()
        .write_all(CLIENT_MSG.as_bytes())
        .context("Socket send failed")?;
    if shutdown {
        conn.get_mut()
            .shutdown(Shutdown::Write)
            .context("Shutdown of writing end failed")?;
    }

    if shutdown {
        conn.read_to_string(&mut buffer)
    } else {
        conn.read_line(&mut buffer)
    }
    .context("Socket receive failed")?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
