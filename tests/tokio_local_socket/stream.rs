use {
    super::util::{NameGen, TestResult},
    color_eyre::eyre::Context,
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::local_socket::tokio::{LocalSocketListener, LocalSocketStream},
    std::{convert::TryInto, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task, try_join},
};

static SERVER_LINE: &[u8] = b"Hello from server!\n";
static SERVER_BYTES: &[u8] = b"Bytes from server!\0";
static CLIENT_LINE: &[u8] = b"Hello from client!\n";
static CLIENT_BYTES: &[u8] = b"Bytes from client!\0";

pub async fn server(name_sender: Sender<String>, num_clients: u32, prefer_namespaced: bool) -> TestResult {
    async fn handle_conn(conn: LocalSocketStream) -> TestResult {
        let (reader, mut writer) = conn.split();
        let mut buffer = Vec::with_capacity(128);
        let mut reader = BufReader::new(reader);

        let read = async {
            reader
                .read_until(b'\n', &mut buffer)
                .await
                .context("First socket receive failed")?;
            assert_eq!(buffer, CLIENT_LINE);
            buffer.clear();

            reader
                .read_until(b'\0', &mut buffer)
                .await
                .context("Second socket receive failed")?;
            assert_eq!(buffer, CLIENT_BYTES);
            TestResult::Ok(())
        };
        let write = async {
            writer
                .write_all(SERVER_LINE)
                .await
                .context("First socket send failed")?;

            writer
                .write_all(SERVER_BYTES)
                .await
                .context("First socket send failed")?;
            TestResult::Ok(())
        };
        try_join!(read, write)?;
        Ok(())
    }

    let (name, listener) = NameGen::new_auto(make_id!(), prefer_namespaced)
        .find_map(|nm| {
            let l = match LocalSocketListener::bind(&*nm) {
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
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };
        tasks.push(task::spawn(handle_conn(conn)));
    }
    for task in tasks {
        task.await
            .context("Server task panicked")?
            .context("Server task returned early with error")?;
    }
    Ok(())
}
pub async fn client(name: Arc<String>) -> TestResult {
    let mut buffer = Vec::with_capacity(128);

    let (reader, mut writer) = LocalSocketStream::connect(name.as_str())
        .await
        .context("Connect failed")?
        .split();
    let mut reader = BufReader::new(reader);

    let read = async {
        reader
            .read_until(b'\n', &mut buffer)
            .await
            .context("First socket receive failed")?;
        assert_eq!(buffer, SERVER_LINE);
        buffer.clear();

        reader
            .read_until(b'\0', &mut buffer)
            .await
            .context("Second socket receive failed")?;
        assert_eq!(buffer, SERVER_BYTES);
        TestResult::Ok(())
    };
    let write = async {
        writer
            .write_all(CLIENT_LINE)
            .await
            .context("First socket send failed")?;

        writer
            .write_all(CLIENT_BYTES)
            .await
            .context("Second socket send failed")?;
        TestResult::Ok(())
    };
    try_join!(read, write)?;
    Ok(())
}
