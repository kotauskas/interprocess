use super::{util::*, NameGen};
use color_eyre::eyre::Context;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{
    io::{BufRead, BufReader, Write},
    str,
    sync::{mpsc::Sender, Arc},
};

fn msg(server: bool, nts: bool) -> Box<str> {
    message(None, server, Some(['\n', '\0'][nts as usize]))
}

pub fn server(name_sender: Sender<Arc<str>>, num_clients: u32, prefer_namespaced: bool) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut NameGen::new_auto(make_id!(), prefer_namespaced), |nm| {
        LocalSocketListener::bind(nm)
    })?;

    let _ = name_sender.send(name);

    for _ in 0..num_clients {
        let mut conn = listener.accept().context("accept failed").map(BufReader::new)?;
        read(&mut conn, msg(false, false), 0)?;
        write(&mut conn, msg(true, false), 0)?;
        read(&mut conn, msg(false, true), 0)?;
        write(&mut conn, msg(true, true), 0)?;
    }
    Ok(())
}
pub fn client(name: &str) -> TestResult {
    let mut conn = LocalSocketStream::connect(name)
        .context("connect failed")
        .map(BufReader::new)?;
    write(&mut conn, msg(false, false), 0)?;
    read(&mut conn, msg(true, false), 0)?;
    write(&mut conn, msg(false, true), 0)?;
    read(&mut conn, msg(true, true), 0)
}

fn read(conn: &mut BufReader<LocalSocketStream>, exp: impl AsRef<str>, nr: u8) -> TestResult {
    let exp_ = exp.as_ref();
    let term = *exp_.as_bytes().last().unwrap();
    let fs = ["first", "second"][nr as usize];

    let mut buffer = Vec::with_capacity(exp_.len());
    conn.read_until(term, &mut buffer)
        .with_context(|| format!("{} receive failed", fs))?;
    ensure_eq!(
        str::from_utf8(&buffer).with_context(|| format!("{} receive wasn't valid UTF-8", fs))?,
        exp_,
    );
    Ok(())
}
fn write(conn: &mut BufReader<LocalSocketStream>, msg: impl AsRef<str>, nr: u8) -> TestResult {
    let fs = ["first", "second"][nr as usize];
    conn.get_mut()
        .write_all(msg.as_ref().as_bytes())
        .with_context(|| format!("{} socket send failed", fs))
}
