//! Test utilities for allocating an address for the server and then spawning clients to connect to it.
#![allow(dead_code)]

mod choke;
use choke::*;

mod xorshift;
pub use xorshift::*;

mod namegen;
pub use namegen::*;

#[cfg(feature = "tokio_support")]
pub mod tokio;

const NUM_CLIENTS: u32 = 20;
const NUM_CONCURRENT_CLIENTS: u32 = 4;

use {
    std::{
        sync::{
            mpsc::{channel, /*Receiver,*/ Sender},
            Arc,
        },
        thread,
    },
    to_method::*,
};

pub type TestResult = anyhow::Result<()>;

/// Waits for the leader closure to reach a point where it sends a message for the follower closure, then runs the follower. Captures Anyhow errors on both sides and panics if any occur, reporting which side produced the error.
pub fn drive_pair<T, Ld, Fl>(leader: Ld, leader_name: &str, follower: Fl, follower_name: &str)
where
    T: Send + 'static,
    Ld: FnOnce(Sender<T>) -> TestResult + Send + 'static,
    Fl: FnOnce(T) -> TestResult,
{
    let (sender, receiver) = channel();

    let ltname = leader_name.to_lowercase();
    let leading_thread = thread::Builder::new()
        .name(ltname)
        .spawn(move || leader(sender))
        // Lazy .expect()
        .unwrap_or_else(|e| panic!("{} thread launch failed: {}", leader_name, e));

    if let Ok(msg) = receiver.recv() {
        // If the leader reached the send point, proceed with the follower code
        let fres = follower(msg);
        if let Err(e) = fres {
            panic!("{} exited early with error: {:#}", follower_name, e);
        }
    }
    match leading_thread.join() {
        Err(_) => panic!("{} panicked", leader_name),
        Ok(Err(error)) => panic!("{} exited early with error: {:#}", leader_name, error),
        _ => (),
    }
}
pub fn drive_server_and_multiple_clients<T, Srv, Clt>(server: Srv, client: Clt)
where
    T: Send + Sync + 'static,
    Srv: FnOnce(Sender<T>, u32) -> TestResult + Send + 'static,
    Clt: Fn(Arc<T>) -> TestResult + Send + Sync + 'static,
{
    let choke = Choke::new(NUM_CONCURRENT_CLIENTS);

    let client = Arc::new(client);
    let client_wrapper = move |msg| {
        let msg = Arc::new(msg);
        let mut client_threads = Vec::with_capacity(NUM_CLIENTS.try_to().unwrap());
        for _ in 0..NUM_CLIENTS {
            let choke_guard = choke.take();
            let clientc = Arc::clone(&client);
            let msgc = Arc::clone(&msg);
            let jhndl = thread::spawn(move || {
                let _cg = choke_guard; // Send to other thread to drop when client finishes
                clientc(msgc)
            });
            client_threads.push(jhndl);
        }
        for client in client_threads {
            client.join().expect("Client panicked")?; // Early-return the first error
        }
        Ok::<(), anyhow::Error>(())
    };
    let server_wrapper = move |sender: Sender<T>| server(sender, NUM_CLIENTS);

    drive_pair(server_wrapper, "Server", client_wrapper, "Client");
}
