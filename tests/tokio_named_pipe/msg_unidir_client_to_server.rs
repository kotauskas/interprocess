use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncReadExt, AsyncWriteExt},
    interprocess::os::windows::named_pipe::{
        tokio::{MsgReaderPipeStream, MsgWriterPipeStream, PipeListenerOptionsExt},
        PipeListenerOptions, PipeMode,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc, time::Duration},
    tokio::{sync::oneshot::Sender, task, time::sleep},
};
const MSG_1: &[u8] = b"Client message 1";
const MSG_2: &[u8] = b"Client message 2";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(mut conn: MsgReaderPipeStream) -> TestResult {
        let (mut buf1, mut buf2) = ([0; MSG_1.len()], [0; MSG_2.len()]);

        let read = conn
            .read(&mut buf1)
            .await
            .context("First pipe receive failed")?;
        assert_eq!(read, MSG_1.len());
        assert_eq!(&buf1[0..read], MSG_1);

        let read = conn
            .read(&mut buf2)
            .await
            .context("Second pipe receive failed")?;
        assert_eq!(read, MSG_2.len());
        assert_eq!(&buf2[0..read], MSG_2);

        Ok(())
    }

    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create_tokio::<MsgReaderPipeStream>()
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
        match MsgWriterPipeStream::connect(name.as_str()) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(10)).await;
                continue;
            }
            not_busy => break not_busy,
        }
    }
    .context("Connect failed")?;

    let written = conn.write(MSG_1).await.context("First pipe send failed")?;
    assert_eq!(written, MSG_1.len());

    let written = conn.write(MSG_2).await.context("Second pipe send failed")?;
    assert_eq!(written, MSG_2.len());

    Ok(())
}
