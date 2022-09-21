use {
    super::{TestResult, NUM_CLIENTS, NUM_CONCURRENT_CLIENTS},
    anyhow::Context,
    std::{convert::TryInto, future::Future, sync::Arc},
    tokio::{
        sync::{
            oneshot::{channel, Sender},
            Semaphore,
        },
        task, try_join,
    },
};

/// Waits for the leader closure to reach a point where it sends a message for the follower closure, then runs the follower. Captures Anyhow errors on both sides and panics if any occur, reporting which side produced the error.
pub async fn drive_pair<T, Ld, Ldf, Fl, Flf>(
    leader: Ld,
    leader_name: &str,
    follower: Fl,
    follower_name: &str,
) -> TestResult
where
    Ld: FnOnce(Sender<T>) -> Ldf,
    Ldf: Future<Output = TestResult>,
    Fl: FnOnce(T) -> Flf,
    Flf: Future<Output = TestResult>,
{
    let (sender, receiver) = channel();

    let leading_task = async {
        leader(sender)
            .await
            .with_context(|| format!("{} exited early with error", leader_name))
    };
    let following_task = async {
        let msg = receiver.await?;
        follower(msg)
            .await
            .with_context(|| format!("{} exited early with error", follower_name))
    };
    try_join!(leading_task, following_task).map(|((), ())| ())
}

pub async fn drive_server_and_multiple_clients<T, Srv, Srvf, Clt, Cltf>(
    server: Srv,
    client: Clt,
) -> TestResult
where
    T: Send + Sync + 'static,
    Srv: FnOnce(Sender<T>, u32) -> Srvf + Send + 'static,
    Srvf: Future<Output = TestResult>,
    Clt: Fn(Arc<T>) -> Cltf + Send + Sync + 'static,
    Cltf: Future<Output = TestResult> + Send,
{
    let client_wrapper = move |msg| async {
        let client = Arc::new(client);
        let choke = Arc::new(Semaphore::new(NUM_CONCURRENT_CLIENTS.try_into().unwrap()));

        let msg = Arc::new(msg);
        let mut client_tasks = Vec::with_capacity(NUM_CLIENTS.try_into().unwrap());
        for _ in 0..NUM_CLIENTS {
            let permit = Arc::clone(&choke).acquire_owned().await.unwrap();
            let clientc = Arc::clone(&client);
            let msgc = Arc::clone(&msg);
            let jhndl = task::spawn(async move {
                let _prm = permit; // Send to other thread to drop when client finishes
                clientc(msgc).await
            });
            client_tasks.push(jhndl);
        }
        for client in client_tasks {
            client.await.expect("Client panicked")?; // Early-return the first error
        }
        Ok::<(), anyhow::Error>(())
    };
    let server_wrapper = move |sender: Sender<T>| server(sender, NUM_CLIENTS);

    drive_pair(server_wrapper, "Server", client_wrapper, "Client").await
}
