use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    interprocess::{
        os::windows::named_pipe::{pipe_mode, PipeListenerOptions, PipeMode, SendPipeStream},
        reliable_recv_msg::*,
    },
    std::{
        ffi::OsStr,
        io,
        sync::{mpsc::Sender, Arc},
    },
};

const MSG_1: &[u8] = b"Client message 1";
const MSG_2: &[u8] = b"Client message 2";

pub fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create_recv_only::<pipe_mode::Messages>()
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
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };

        let (mut buf1, mut buf2) = ([0; MSG_1.len()], [0; MSG_2.len()]);

        let rslt = conn.recv(&mut buf1).context("First pipe receive failed")?;
        assert_eq!(rslt.size(), MSG_1.len());
        assert_eq!(rslt.borrow_to_size(&buf1), MSG_1);

        let rslt = conn.recv(&mut buf2).context("Second pipe receive failed")?;
        assert_eq!(rslt.size(), MSG_2.len());
        assert_eq!(rslt.borrow_to_size(&buf1), MSG_2);
    }

    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let conn = SendPipeStream::<pipe_mode::Messages>::connect(name.as_str()).context("Connect failed")?;

    let sent = conn.send(MSG_1).context("First pipe send failed")?;
    assert_eq!(sent, MSG_1.len());

    let sent = conn.send(MSG_2).context("Second pipe send failed")?;
    assert_eq!(sent, MSG_2.len());

    conn.flush()?;

    Ok(())
}
