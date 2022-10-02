use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    interprocess::os::windows::named_pipe::{DuplexMsgPipeStream, PipeListenerOptions, PipeMode},
    std::{
        ffi::OsStr,
        io::{self, prelude::*},
        sync::{mpsc::Sender, Arc},
    },
};

const SERVER_MSG_1: &[u8] = b"Server message 1";
const SERVER_MSG_2: &[u8] = b"Server message 2";

const CLIENT_MSG_1: &[u8] = b"Client message 1";
const CLIENT_MSG_2: &[u8] = b"Client message 2";

pub fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create::<DuplexMsgPipeStream>()
            {
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
            Ok(c) => c,
            Err(e) => {
                eprintln!("Incoming connection failed: {}", e);
                continue;
            }
        };

        let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

        let read = conn.read(&mut buf1).context("First pipe receive failed")?;
        assert_eq!(read, CLIENT_MSG_1.len());
        assert_eq!(&buf1[0..read], CLIENT_MSG_1);

        let read = conn.read(&mut buf2).context("Second pipe receive failed")?;
        assert_eq!(read, CLIENT_MSG_1.len());
        assert_eq!(&buf1[0..read], CLIENT_MSG_1);

        let written = conn.write(SERVER_MSG_1).context("First pipe send failed")?;
        assert_eq!(written, SERVER_MSG_1.len());

        let written = conn
            .write(SERVER_MSG_2)
            .context("Second pipe send failed")?;
        assert_eq!(written, SERVER_MSG_2.len());
    }

    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

    let mut conn = DuplexMsgPipeStream::connect(name.as_str()).context("Connect failed")?;

    let written = conn.write(CLIENT_MSG_1).context("First pipe send failed")?;
    assert_eq!(written, CLIENT_MSG_1.len());

    let written = conn
        .write(CLIENT_MSG_2)
        .context("Second pipe send failed")?;
    assert_eq!(written, CLIENT_MSG_2.len());

    let read = conn.read(&mut buf1).context("First pipe receive failed")?;
    assert_eq!(read, SERVER_MSG_1.len());
    assert_eq!(&buf1[0..read], SERVER_MSG_1);

    let read = conn.read(&mut buf2).context("Second pipe receive failed")?;
    assert_eq!(read, SERVER_MSG_1.len());
    assert_eq!(&buf1[0..read], SERVER_MSG_1);

    Ok(())
}
