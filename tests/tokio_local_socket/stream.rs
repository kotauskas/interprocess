use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::local_socket::tokio::{LocalSocketListener, LocalSocketStream},
    std::{convert::TryInto, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task, try_join},
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub async fn server(
    name_sender: Sender<String>,
    num_clients: u32,
    prefer_namespaced: bool,
) -> TestResult {
    async fn handle_conn(conn: LocalSocketStream) -> TestResult {
        let (reader, mut writer) = conn.into_split();
        let mut buffer = String::with_capacity(128);
        let mut reader = BufReader::new(reader);

        let read = async {
            reader
                .read_line(&mut buffer)
                .await
                .context("Socket receive failed")
        };
        let write = async {
            writer
                .write_all(SERVER_MSG.as_bytes())
                .await
                .context("Socket send failed")
        };
        try_join!(read, write)?;

        assert_eq!(buffer, CLIENT_MSG);
        Ok(())
    }

    let (name, listener) = NameGen::new_auto(prefer_namespaced)
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
                eprintln!("Incoming connection failed: {}", e);
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
    let mut buffer = String::with_capacity(128);

    let (reader, mut writer) = LocalSocketStream::connect(name.as_str())
        .await
        .context("Connect failed")?
        .into_split();
    let mut reader = BufReader::new(reader);

    let read = async {
        reader
            .read_line(&mut buffer)
            .await
            .context("Socket receive failed")
    };
    let write = async {
        writer
            .write_all(CLIENT_MSG.as_bytes())
            .await
            .context("Socket send failed")
    };
    try_join!(read, write)?;

    assert_eq!(buffer, SERVER_MSG);

    Ok(())
}
