use {
    super::util::{NameGen, TestResult},
    color_eyre::eyre::{bail, Context},
    interprocess::{
        os::windows::named_pipe::{
            pipe_mode,
            tokio::{DuplexPipeStream, PipeListenerOptionsExt},
            PipeListenerOptions, PipeMode,
        },
        reliable_recv_msg::AsyncReliableRecvMsgExt,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task, try_join},
};
// TODO untangle imports, use listen_and_pick_name

const SERVER_MSG_1: &[u8] = b"First server message";
const SERVER_MSG_2: &[u8] = b"Second server message";

const CLIENT_MSG_1: &[u8] = b"First client message";
const CLIENT_MSG_2: &[u8] = b"Second client message";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: DuplexPipeStream<pipe_mode::Messages>) -> TestResult {
        let (reader, writer) = conn.split();
        let (mut buf1, mut buf2) = ([0; CLIENT_MSG_1.len()], [0; CLIENT_MSG_2.len()]);

        let recv = async {
            let rslt = (&reader).recv(&mut buf1).await.context("first pipe receive failed")?;
            assert_eq!(rslt.size(), CLIENT_MSG_1.len());
            assert_eq!(rslt.borrow_to_size(&buf1), CLIENT_MSG_1);

            let rslt = (&reader).recv(&mut buf2).await.context("second pipe receive failed")?;
            assert_eq!(rslt.size(), CLIENT_MSG_2.len());
            assert_eq!(rslt.borrow_to_size(&buf2), CLIENT_MSG_2);

            TestResult::Ok(())
        };
        let send = async {
            let sent = writer.send(SERVER_MSG_1).await.context("pipe send failed")?;
            assert_eq!(sent, SERVER_MSG_1.len());

            let sent = writer.send(SERVER_MSG_2).await.context("pipe send failed")?;
            assert_eq!(sent, SERVER_MSG_2.len());

            writer.flush().await.context("flush failed")?;

            TestResult::Ok(())
        };
        try_join!(recv, send)?;

        Ok(())
    }

    let (name, listener) = NameGen::new(make_id!(), true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create_tokio_duplex::<pipe_mode::Messages>()
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
    let (reader, writer) = DuplexPipeStream::<pipe_mode::Messages>::connect(name.as_str())
        .await
        .context("connect failed")?
        .split();

    let (mut buf1, mut buf2) = ([0; SERVER_MSG_1.len()], [0; SERVER_MSG_2.len()]);

    let recv = async {
        let rslt = (&reader).recv(&mut buf1).await.context("first pipe receive failed")?;
        assert_eq!(rslt.size(), SERVER_MSG_1.len());
        assert_eq!(rslt.borrow_to_size(&buf1), SERVER_MSG_1);

        let rslt = (&reader).recv(&mut buf2).await.context("second pipe receive failed")?;
        assert_eq!(rslt.size(), SERVER_MSG_2.len());
        assert_eq!(rslt.borrow_to_size(&buf2), SERVER_MSG_2);

        TestResult::Ok(())
    };
    let send = async {
        let sent = writer.send(CLIENT_MSG_1).await.context("first pipe send failed")?;
        assert_eq!(sent, CLIENT_MSG_1.len());

        let sent = writer.send(CLIENT_MSG_2).await.context("second pipe send failed")?;
        assert_eq!(sent, CLIENT_MSG_2.len());

        writer.flush().await.context("flush failed")?;

        TestResult::Ok(())
    };
    try_join!(recv, send)?;

    Ok(())
}
