use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncReadExt, AsyncWriteExt},
    interprocess::os::windows::named_pipe::{
        tokio::{DuplexMsgPipeStream, PipeListenerOptionsExt},
        PipeListenerOptions, PipeMode,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc, time::Duration},
    tokio::{sync::oneshot::Sender, task, time::sleep, try_join},
};

const SERVER_MSG_1: &[u8] = b"Server message 1";
const SERVER_MSG_2: &[u8] = b"Server message 2";

const CLIENT_MSG_1: &[u8] = b"Client message 1";
const CLIENT_MSG_2: &[u8] = b"Client message 2";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: DuplexMsgPipeStream) -> TestResult {
        let (mut reader, mut writer) = conn.split();
        let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

        let read = async {
            let read = reader
                .read(&mut buf1)
                .await
                .context("First pipe receive failed")?;
            assert_eq!(read, CLIENT_MSG_1.len());
            assert_eq!(&buf1[0..read], CLIENT_MSG_1);

            let read = reader
                .read(&mut buf2)
                .await
                .context("Second pipe receive failed")?;
            assert_eq!(read, CLIENT_MSG_2.len());
            assert_eq!(&buf2[0..read], CLIENT_MSG_2);

            TestResult::Ok(())
        };
        let write = async {
            let written = writer
                .write(SERVER_MSG_1)
                .await
                .context("Pipe send failed")?;
            assert_eq!(written, SERVER_MSG_1.len());

            let written = writer
                .write(SERVER_MSG_2)
                .await
                .context("Pipe send failed")?;
            assert_eq!(written, SERVER_MSG_2.len());

            TestResult::Ok(())
        };
        try_join!(read, write)?;

        Ok(())
    }

    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create_tokio::<DuplexMsgPipeStream>()
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

    let mut tasks = Vec::with_capacity(num_clients.try_into().unwrap());

    for _ in 0..num_clients {
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Incoming connection failed: {}", e);
                continue;
            }
        };
        let task = task::spawn(handle_conn(conn));
        tasks.push(task);
    }
    for task in tasks {
        task.await
            .context("Server task panicked")?
            .context("Server task returned early with error")?;
    }

    Ok(())
}
pub async fn client(name: Arc<String>) -> TestResult {
    let (mut reader, mut writer) = loop {
        match DuplexMsgPipeStream::connect(name.as_str()) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(10)).await;
                continue;
            }
            not_busy => break not_busy,
        }
    }
    .context("Connect failed")?
    .split();

    let (mut buf1, mut buf2) = ([0; SERVER_MSG_1.len()], [0; SERVER_MSG_2.len()]);

    let read = async {
        let read = reader
            .read(&mut buf1)
            .await
            .context("First pipe receive failed")?;
        assert_eq!(read, SERVER_MSG_1.len());
        assert_eq!(&buf1[0..read], SERVER_MSG_1);

        let read = reader
            .read(&mut buf2)
            .await
            .context("Second pipe receive failed")?;
        assert_eq!(read, SERVER_MSG_2.len());
        assert_eq!(&buf2[0..read], SERVER_MSG_2);

        TestResult::Ok(())
    };
    let write = async {
        let written = writer
            .write(CLIENT_MSG_1)
            .await
            .context("First pipe send failed")?;
        assert_eq!(written, CLIENT_MSG_1.len());

        let written = writer
            .write(CLIENT_MSG_2)
            .await
            .context("Second pipe send failed")?;
        assert_eq!(written, CLIENT_MSG_2.len());

        TestResult::Ok(())
    };
    try_join!(read, write)?;

    Ok(())
}
