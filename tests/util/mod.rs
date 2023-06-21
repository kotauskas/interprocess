//! Test utilities for allocating an address for the server and then spawning clients to connect to it.
#![allow(dead_code)]

mod choke;
use choke::*;

mod xorshift;
pub use xorshift::*;

#[macro_use]
mod namegen;
pub use namegen::*;

#[cfg(feature = "tokio")]
pub mod tokio;

const NUM_CLIENTS: u32 = 80;
const NUM_CONCURRENT_CLIENTS: u32 = 6;

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

pub type TestResult = color_eyre::eyre::Result<()>;

/// Waits for the leader closure to reach a point where it sends a message for the follower closure, then runs the follower. Captures Eyre errors on both sides and panics if any occur, reporting which side produced the error.
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
        .unwrap_or_else(|e| panic!("{leader_name} thread launch failed: {e}"));

    if let Ok(msg) = receiver.recv() {
        // If the leader reached the send point, proceed with the follower code
        let fres = follower(msg);
        if let Err(e) = fres {
            panic!("{follower_name} exited early with error: {e:#}");
        }
    }
    match leading_thread.join() {
        Err(_) => panic!("{leader_name} panicked"),
        Ok(Err(error)) => panic!("{leader_name} exited early with error: {error:#}"),
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
        for n in 1..=NUM_CLIENTS {
            let tname = format!("client {n}");
            let clientc = Arc::clone(&client);
            let msgc = Arc::clone(&msg);

            let choke_guard = choke.take();

            let jhndl = thread::Builder::new()
                .name(tname.clone())
                .spawn(move || {
                    let _cg = choke_guard; // Send to other thread to drop when client finishes
                    clientc(msgc)
                })
                .unwrap_or_else(|e| panic!("{tname} thread launch failed: {e}"));
            client_threads.push(jhndl);
        }
        for client in client_threads {
            client.join().expect("Client panicked")?; // Early-return the first error
        }
        Ok::<(), color_eyre::eyre::Error>(())
    };
    let server_wrapper = move |sender: Sender<T>| server(sender, NUM_CLIENTS);

    drive_pair(server_wrapper, "Server", client_wrapper, "Client");
}

pub fn message(server: bool, terminator: Option<char>) -> String {
    let sc = if server { "server" } else { "client" };
    let mut msg = format!("Message from {sc}!");
    if let Some(t) = terminator {
        msg.push(t);
    }
    msg
}
