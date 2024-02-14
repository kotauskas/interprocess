use crate::{
	local_socket::{Listener, Name, Stream},
	tests::util::*,
};
use color_eyre::eyre::WrapErr;
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
	id: &'static str,
	handle_client: fn(Stream) -> TestResult,
	name_sender: Sender<Arc<Name<'static>>>,
	num_clients: u32,
	path: bool,
) -> TestResult {
	let (name, listener) = listen_and_pick_name(&mut namegen_local_socket(id, path), |nm| {
		Listener::bind(nm.borrow())
	})?;
	let _ = name_sender.send(name);
	listener
		.incoming()
		.take(num_clients.try_into().unwrap())
		.try_for_each(|conn| handle_client(conn.opname("accept")?))
}

pub fn handle_client_nosplit(conn: Stream) -> TestResult {
	let mut conn = BufReader::new(conn);
	recv(&mut conn, &msg(false, false), 0)?;
	send(conn.get_mut(), &msg(true, false), 0)?;
	recv(&mut conn, &msg(false, true), 1)?;
	send(conn.get_mut(), &msg(true, true), 1)
}

pub fn handle_client_split(conn: Stream) -> TestResult {
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
	Stream::reunite(recver, sender).opname("reunite")?;
	Ok(())
}

pub fn client_nosplit(name: &Name<'_>) -> TestResult {
	let mut conn = Stream::connect(name.borrow())
		.opname("connect")
		.map(BufReader::new)?;
	send(conn.get_mut(), &msg(false, false), 0)?;
	recv(&mut conn, &msg(true, false), 0)?;
	send(conn.get_mut(), &msg(false, true), 1)?;
	recv(&mut conn, &msg(true, true), 1)
}

pub fn client_split(name: &Name<'_>) -> TestResult {
	let (recver, sender) = Stream::connect(name.borrow()).opname("connect")?.split();

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
	Stream::reunite(recver, sender).opname("reunite")?;
	Ok(())
}

fn recv(conn: &mut dyn BufRead, exp: &str, nr: u8) -> TestResult {
	let term = *exp.as_bytes().last().unwrap();
	let fs = ["first", "second"][nr as usize];

	let mut buffer = Vec::with_capacity(exp.len());
	conn.read_until(term, &mut buffer)
		.wrap_err_with(|| format!("{} receive failed", fs))?;
	ensure_eq!(
		str::from_utf8(&buffer).with_context(|| format!("{} receive wasn't valid UTF-8", fs))?,
		exp,
	);
	Ok(())
}
fn send(conn: &mut dyn Write, msg: &str, nr: u8) -> TestResult {
	let fs = ["first", "second"][nr as usize];
	conn.write_all(msg.as_bytes())
		.with_context(|| format!("{} socket send failed", fs))
}
