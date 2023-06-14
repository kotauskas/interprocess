use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    interprocess::os::windows::named_pipe::{pipe_mode, DuplexPipeStream, PipeListenerOptions},
    std::{
        ffi::OsStr,
        io::{self, prelude::*, BufReader},
        sync::{mpsc::Sender, Arc},
    },
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

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
        .context("Listener bind failed")?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };

        let mut buffer = String::with_capacity(128);

        conn.read_line(&mut buffer).context("Pipe receive failed")?;
        assert_eq!(buffer, CLIENT_MSG);

        conn.get_mut()
            .write_all(SERVER_MSG.as_bytes())
            .context("Pipe send failed")?;
        conn.get_mut().flush().context("Pipe flush failed")?;
    }

    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut conn = DuplexPipeStream::<pipe_mode::Bytes>::connect(name.as_str())
        .context("Connect failed")
        .map(BufReader::new)?;

    conn.get_mut()
        .write_all(CLIENT_MSG.as_bytes())
        .context("Pipe send failed")?;

    conn.read_line(&mut buffer).context("Pipe receive failed")?;
    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
