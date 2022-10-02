use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::os::windows::named_pipe::{
        tokio::{ByteReaderPipeStream, ByteWriterPipeStream, PipeListenerOptionsExt},
        PipeListenerOptions,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc, time::Duration},
    tokio::{sync::oneshot::Sender, task, time::sleep},
};

static MSG: &str = "Hello from client!\n";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: ByteReaderPipeStream) -> TestResult {
        let mut buffer = String::with_capacity(128);
        let mut conn = BufReader::new(conn);

        conn.read_line(&mut buffer)
            .await
            .context("Pipe receive failed")?;

        assert_eq!(buffer, MSG);

        Ok(())
    }

    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .create_tokio::<ByteReaderPipeStream>()
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
    let mut conn = loop {
        match ByteWriterPipeStream::connect(name.as_str()) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(10)).await;
                continue;
            }
            not_busy => break not_busy,
        }
    }
    .context("Connect failed")?;

    conn.write_all(MSG.as_bytes())
        .await
        .context("Pipe send failed")?;

    Ok(())
}
