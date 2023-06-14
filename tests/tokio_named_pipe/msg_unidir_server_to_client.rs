use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    interprocess::{
        os::windows::named_pipe::{
            pipe_mode,
            tokio::{PipeListenerOptionsExt, RecvPipeStream, SendPipeStream},
            PipeListenerOptions, PipeMode,
        },
        reliable_recv_msg::AsyncReliableRecvMsgExt,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task},
};
const MSG_1: &[u8] = b"First server message";
const MSG_2: &[u8] = b"Second server message";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(conn: SendPipeStream<pipe_mode::Messages>) -> TestResult {
        let sent = conn.send(MSG_1).await.context("First pipe send failed")?;
        assert_eq!(sent, MSG_1.len());

        let sent = conn.send(MSG_2).await.context("Second pipe send failed")?;
        assert_eq!(sent, MSG_2.len());

        conn.flush().await.context("Flush failed")?;

        Ok(())
    }

    let (name, listener) = NameGen::new(make_id!(), true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .mode(PipeMode::Messages)
                .create_tokio_send_only::<pipe_mode::Messages>()
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
                eprintln!("Incoming connection failed: {e}");
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
    let conn = RecvPipeStream::<pipe_mode::Messages>::connect(name.as_str())
        .await
        .context("Connect failed")?;

    let (mut buf1, mut buf2) = ([0; MSG_1.len()], [0; MSG_2.len()]);

    let rslt = (&conn).recv(&mut buf1).await.context("First pipe receive failed")?;
    assert_eq!(rslt.size(), MSG_1.len());
    assert_eq!(rslt.borrow_to_size(&buf1), MSG_1);

    let rslt = (&conn).recv(&mut buf2).await.context("Second pipe receive failed")?;
    assert_eq!(rslt.size(), MSG_2.len());
    assert_eq!(rslt.borrow_to_size(&buf2), MSG_2);

    Ok(())
}
