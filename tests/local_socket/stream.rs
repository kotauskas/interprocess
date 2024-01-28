use crate::{
    local_socket::{LocalSocketListener, LocalSocketStream},
    tests::util::*,
};
use color_eyre::eyre::Context;
use std::{
    io::{BufRead, BufReader, Write},
    str,
    sync::{mpsc::Sender, Arc},
    thread,
};

fn msg(server: bool, nts: bool) -> Box<str> {
    message(None, server, Some(['\n', '\0'][nts as usize]))
}

pub fn server(
    handle_client: fn(LocalSocketStream) -> TestResult,
    name_sender: Sender<Arc<str>>,
    num_clients: u32,
    namespaced: bool,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new(make_id!(), namespaced), |nm| {
        LocalSocketListener::bind(nm)
    })?;
    let _ = name_sender.send(name);
    listener
        .incoming()
        .take(num_clients.try_into().unwrap())
        .try_for_each(|conn| handle_client(conn.context("accept failed")?))
}

pub fn handle_client_nosplit(conn: LocalSocketStream) -> TestResult {
    let mut conn = BufReader::new(conn);
    recv(&mut conn, &msg(false, false), 0)?;
    send(conn.get_mut(), &msg(true, false), 0)?;
    recv(&mut conn, &msg(false, true), 1)?;
    send(conn.get_mut(), &msg(true, true), 1)
}

pub fn handle_client_split(conn: LocalSocketStream) -> TestResult {
    let (recver, sender) = conn.split();

    let recv = thread::spawn(move || {
        let mut recver = BufReader::new(recver);
        recv(&mut recver, &msg(true, false), 0)?;
        recv(&mut recver, &msg(true, true), 1)?;
        TestResult::<_>::Ok(recver.into_inner())
    });
    let send = thread::spawn(move || {
        let mut sender = sender;
        send(&mut sender, &msg(false, false), 0)?;
        send(&mut sender, &msg(false, true), 1)?;
        TestResult::<_>::Ok(sender)
    });

    let recver = recv.join().unwrap()?;
    let sender = send.join().unwrap()?;
    LocalSocketStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}

pub fn client_nosplit(name: &str) -> TestResult {
    let mut conn =
        LocalSocketStream::connect(name).context("connect failed").map(BufReader::new)?;
    send(conn.get_mut(), &msg(false, false), 0)?;
    recv(&mut conn, &msg(true, false), 0)?;
    send(conn.get_mut(), &msg(false, true), 1)?;
    recv(&mut conn, &msg(true, true), 1)
}

pub fn client_split(name: &str) -> TestResult {
    let (recver, sender) = LocalSocketStream::connect(name).context("connect failed")?.split();

    let recv = thread::spawn(move || {
        let mut recver = BufReader::new(recver);
        recv(&mut recver, &msg(false, false), 0)?;
        recv(&mut recver, &msg(false, true), 1)?;
        TestResult::<_>::Ok(recver.into_inner())
    });
    let send = thread::spawn(move || {
        let mut sender = sender;
        send(&mut sender, &msg(true, false), 0)?;
        send(&mut sender, &msg(true, true), 1)?;
        TestResult::<_>::Ok(sender)
    });

    let recver = recv.join().unwrap()?;
    let sender = send.join().unwrap()?;
    LocalSocketStream::reunite(recver, sender).context("reunite failed")?;
    Ok(())
}

fn recv(conn: &mut dyn BufRead, exp: &str, nr: u8) -> TestResult {
    let term = *exp.as_bytes().last().unwrap();
    let fs = ["first", "second"][nr as usize];

    let mut buffer = Vec::with_capacity(exp.len());
    conn.read_until(term, &mut buffer).with_context(|| format!("{} receive failed", fs))?;
    ensure_eq!(
        str::from_utf8(&buffer).with_context(|| format!("{} receive wasn't valid UTF-8", fs))?,
        exp,
    );
    Ok(())
}
fn send(conn: &mut dyn Write, msg: &str, nr: u8) -> TestResult {
    let fs = ["first", "second"][nr as usize];
    conn.write_all(msg.as_bytes()).with_context(|| format!("{} socket send failed", fs))
}
