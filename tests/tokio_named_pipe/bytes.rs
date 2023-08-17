use {
    super::util::{NameGen, TestResult},
    color_eyre::eyre::{bail, Context},
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::os::windows::named_pipe::{
        pipe_mode,
        tokio::{DuplexPipeStream, PipeListenerOptionsExt},
        PipeListenerOptions,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task, try_join},
};
// TODO context instead of bail, ensure_eq, untangle imports, use listen_and_pick_name

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: DuplexPipeStream<pipe_mode::Bytes>) -> TestResult {
        let (reader, mut writer) = conn.split();
        let mut buffer = String::with_capacity(128);
        let mut reader = BufReader::new(reader);

        let recv = async { reader.read_line(&mut buffer).await.context("pipe receive failed") };
        let send = async {
            writer
                .write_all(SERVER_MSG.as_bytes())
                .await
                .context("pipe send failed")
        };
        try_join!(recv, send)?;

        assert_eq!(buffer, CLIENT_MSG);

        Ok(())
    }

    let (name, listener) = NameGen::new(make_id!(), true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .create_tokio_duplex::<pipe_mode::Bytes>()
            {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("listener bind failed")?;

    let _ = name_sender.send(name);

    let mut tasks = Vec::with_capacity(num_clients.try_into().unwrap());

    for _ in 0..num_clients {
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => bail!("incoming connection failed: {e}"),
        };
        let task = task::spawn(handle_conn(conn));
        tasks.push(task);
    }
    for task in tasks {
        task.await
            .context("server task panicked")?
            .context("server task returned early with error")?;
    }

    Ok(())
}
pub async fn client(name: Arc<String>) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let (reader, mut writer) = DuplexPipeStream::<pipe_mode::Bytes>::connect(name.as_str())
        .await
        .context("connect failed")?
        .split();

    let mut reader = BufReader::new(reader);

    let read = async { reader.read_line(&mut buffer).await.context("pipe receive failed") };
    let write = async {
        writer
            .write_all(CLIENT_MSG.as_bytes())
            .await
            .context("pipe send failed")
    };
    try_join!(read, write)?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
