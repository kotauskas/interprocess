use super::util::*;
use color_eyre::eyre::{bail, Context};
use interprocess::os::unix::udsocket::{UdSocket, UdStream, UdStreamListener};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::Shutdown,
    sync::{mpsc::Sender, Arc},
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub(super) fn run_with_namegen(namegen: NameGen) {
    drive_server_and_multiple_clients(move |snd, nc| server(snd, nc, namegen, false), |nm| client(nm, false));
    drive_server_and_multiple_clients(move |snd, nc| server(snd, nc, namegen, true), |nm| client(nm, true));
}

fn server(name_sender: Sender<String>, num_clients: u32, mut namegen: NameGen, shutdown: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut namegen, |nm| UdStreamListener::bind(nm))?;

    let _ = name_sender.send(name);

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => bail!("incoming connection failed: {e}"),
        };

        if shutdown {
            conn.read_to_string(&mut buffer)
        } else {
            conn.read_line(&mut buffer)
        }
        .context("socket receive failed")?;

        conn.get_mut()
            .write_all(SERVER_MSG.as_bytes())
            .context("socket send failed")?;
        if shutdown {
            conn.get_mut()
                .shutdown(Shutdown::Write)
                .context("shutdown of writing end failed")?;
        }

        assert_eq!(buffer, CLIENT_MSG);
        buffer.clear();
    }
    Ok(())
}

fn client(name: Arc<String>, shutdown: bool) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let conn = UdStream::connect(name.as_str()).context("connect failed")?;
    let mut conn = BufReader::new(conn);

    conn.get_mut()
        .write_all(CLIENT_MSG.as_bytes())
        .context("socket send failed")?;
    if shutdown {
        conn.get_mut()
            .shutdown(Shutdown::Write)
            .context("shutdown of writing end failed")?;
    }

    if shutdown {
        conn.read_to_string(&mut buffer)
    } else {
        conn.read_line(&mut buffer)
    }
    .context("socket receive failed")?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
