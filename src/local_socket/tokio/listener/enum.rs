use super::r#trait;
use crate::local_socket::{tokio::Stream, Name};
#[cfg(windows)]
use crate::os::windows::named_pipe::local_socket::tokio as np_impl;
use std::io;
#[cfg(unix)]
use {crate::os::unix::uds_local_socket::tokio as uds_impl, std::os::unix::prelude::*};

impmod! { local_socket::dispatch_tokio as dispatch }

// TODO borrowed split in examples

mkenum!(
/// Tokio-based local socket server, listening for connections.
///
/// [Name reclamation](super::super::Stream#name-reclamation) is performed by default on
/// backends that necessitate it.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::local_socket::{
/// 	tokio::{prelude::*, Listener, Stream},
/// 	NameTypeSupport, ToFsName, ToNsName,
/// };
/// use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, try_join};
/// use std::io;
///
/// // Describe the things we do when we've got a connection ready.
/// async fn handle_conn(conn: Stream) -> io::Result<()> {
/// 	let mut recver = BufReader::new(&conn);
/// 	let mut sender = &conn;
///
/// 	// Allocate a sizeable buffer for receiving.
/// 	// This size should be big enough and easy to find for the allocator.
/// 	let mut buffer = String::with_capacity(128);
///
/// 	// Describe the send operation as sending our whole message.
/// 	let send = sender.write_all(b"Hello from server!\n");
/// 	// Describe the receive operation as receiving a line into our big buffer.
/// 	let recv = recver.read_line(&mut buffer);
///
/// 	// Run both operations concurrently.
/// 	try_join!(recv, send)?;
///
/// 	// Produce our output!
/// 	println!("Client answered: {}", buffer.trim());
/// 	Ok(())
/// }
///
/// // Pick a name. There isn't a helper function for this, mostly because it's largely unnecessary:
/// // in Rust, `match` is your concise, readable and expressive decision making construct.
/// let (name, printname) = {
/// 	// This scoping trick allows us to nicely contain the import inside the `match`, so that if
/// 	// any imports of variants named `Both` happen down the line, they won't collide with the
/// 	// enum we're working with here. Maybe someone should make a macro for this.
/// 	use NameTypeSupport::*;
/// 	match NameTypeSupport::query() {
/// 		OnlyFs => {
/// 			let pn = "/tmp/example.sock";
/// 			(pn.to_fs_name()?, pn)
/// 		},
/// 		OnlyNs | Both => {
/// 			let pn = "example.sock";
/// 			(pn.to_ns_name()?, pn)
/// 		},
/// 	}
/// };
///
/// // Bind our listener.
/// let listener = match Listener::bind(name) {
/// 	Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
/// 		// When a program that uses a file-type socket name terminates its socket server without
/// 		// deleting the file, a "corpse socket" remains, which can neither be connected to nor
/// 		// reused by a new listener. Normally, Interprocess takes care of this on affected
/// 		// platforms by deleting the socket file when the listener is dropped. (This is
/// 		// vulnerable to all sorts of races and thus can be disabled.)
/// 		//
/// 		// There are multiple ways this error can be handled, if it occurs, but when the
/// 		// listener only comes from Interprocess, it can be assumed that its previous instance
/// 		// either has crashed or simply hasn't exited yet. In this example, we leave cleanup up
/// 		// to the user, but in a real application, you usually don't want to do that.
/// 		eprintln!(
/// 			"
///Error: could not start server because the socket file is occupied. Please check if {printname}
///is in use by another process and try again."
/// 		);
/// 		return Err(e.into());
/// 	}
/// 	x => x?,
/// };
///
/// // The syncronization between the server and client, if any is used, goes here.
/// eprintln!("Server running at {printname}");
///
/// // Set up our loop boilerplate that processes our incoming connections.
/// loop {
/// 	// Sort out situations when establishing an incoming connection caused an error.
/// 	let conn = match listener.accept().await {
/// 		Ok(c) => c,
/// 		Err(e) => {
/// 			eprintln!("There was an error with an incoming connection: {e}");
/// 			continue;
/// 		}
/// 	};
///
/// 	// Spawn new parallel asynchronous tasks onto the Tokio runtime
/// 	// and hand the connection over to them so that multiple clients
/// 	// could be processed simultaneously in a lightweight fashion.
/// 	tokio::spawn(async move {
/// 		// The outer match processes errors that happen when we're
/// 		// connecting to something. The inner if-let processes errors that
/// 		// happen during the connection.
/// 		if let Err(e) = handle_conn(conn).await {
/// 			eprintln!("Error while handling connection: {e}");
/// 		}
/// 	});
/// }
/// # Ok(()) }
/// ```
Listener);

impl r#trait::Listener for Listener {
	type Stream = Stream;

	#[inline]
	fn bind(name: Name<'_>) -> io::Result<Self> {
		dispatch::bind(name)
	}
	#[inline]
	fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Self> {
		dispatch::bind_without_name_reclamation(name)
	}
	#[inline]
	async fn accept(&self) -> io::Result<Stream> {
		dispatch!(Self: x in self => x.accept())
			.await
			.map(Stream::from)
	}
	#[inline]
	fn do_not_reclaim_name_on_drop(&mut self) {
		dispatch!(Self: x in self => x.do_not_reclaim_name_on_drop())
	}
}

#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
impl TryFrom<OwnedFd> for Listener {
	type Error = <uds_impl::Listener as TryFrom<OwnedFd>>::Error;
	fn try_from(fd: OwnedFd) -> Result<Self, Self::Error> {
		uds_impl::Listener::try_from(fd).map(Self::UdSocket)
	}
}

#[cfg(unix)]
dispatch_as_handle!(Listener);

// TODO handle ops (currently Unix-only)
