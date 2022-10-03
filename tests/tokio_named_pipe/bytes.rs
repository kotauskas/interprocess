use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::os::windows::named_pipe::{
        tokio::{DuplexBytePipeStream, PipeListenerOptionsExt},
        PipeListenerOptions,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc, time::Duration},
    tokio::{sync::oneshot::Sender, task, time::sleep, try_join},
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: DuplexBytePipeStream) -> TestResult {
        let (reader, mut writer) = conn.split();
        let mut buffer = String::with_capacity(128);
        let mut reader = BufReader::new(reader);

        let read = async {
            reader
                .read_line(&mut buffer)
                .await
                .context("Pipe receive failed")
        };
        let write = async {
            writer
                .write_all(SERVER_MSG.as_bytes())
                .await
                .context("Pipe send failed")
        };
        try_join!(read, write)?;

        assert_eq!(buffer, CLIENT_MSG);

        Ok(())
    }

    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .create_tokio::<DuplexBytePipeStream>()
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
    let mut buffer = String::with_capacity(128);

    let (reader, mut writer) = loop {
        match DuplexBytePipeStream::connect(name.as_str()) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(10)).await;
                continue;
            }
            not_busy => break not_busy,
        }
    }
    .context("Connect failed")?
    .split();

    let mut reader = BufReader::new(reader);

    let read = async {
        reader
            .read_line(&mut buffer)
            .await
            .context("Pipe receive failed")
    };
    let write = async {
        writer
            .write_all(CLIENT_MSG.as_bytes())
            .await
            .context("Pipe send failed")
    };
    try_join!(read, write)?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
