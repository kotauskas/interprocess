use super::r#trait;
use crate::local_socket::{Name, Stream};
use std::io;
#[cfg(unix)]
use {crate::os::unix::uds_local_socket as uds_impl, std::os::unix::prelude::*};
#[cfg(windows)]
use {crate::os::windows::named_pipe::local_socket as np_impl, std::os::windows::prelude::*};

impmod! {local_socket::dispatch,
	self,
}

mkenum!(
/// Local socket server, listening for connections.
///
/// # Name reclamation
/// *This section only applies to Unix domain sockets.*
///
/// When a Unix domain socket listener is closed, its associated socket file is not automatically
/// deleted. Instead, it remains on the filesystem in a zombie state, neither accepting connections
/// nor allowing a new listener to reuse it – [`bind()`](Self::bind) will return
/// [`AddrInUse`](io::ErrorKind::AddrInUse) unless it is deleted manually.
///
/// Interprocess implements *automatic name reclamation* via: when the local socket listener is
/// dropped, it performs [`std::fs::remove_file()`] (i.e. `unlink()`) with the path that was
/// originally passed to [`bind()`](Self::bind), allowing for subsequent reuse of the local socket
/// name.
///
/// If the program crashes in a way that doesn't unwind the stack, the deletion will not occur and
/// the socket file will linger on the filesystem, in which case manual deletion will be necessary.
/// Identially, the automatic name reclamation mechanism can be opted out of via
/// [`.do_not_reclaim_name_on_drop()`](Self::do_not_reclaim_name_on_drop) or
/// [`bind_without_name_reclamation()`](Self::bind_without_name_reclamation).
///
/// Note that the socket file can be unlinked by other programs at any time, retaining the inode the
/// listener is bound to but making it inaccessible to peers if it was at its last hardlink. If that
/// happens and another listener takes the same path before the first one performs name reclamation,
/// the socket file deletion wouldn't correspond to the listener being closed, instead deleting the
/// socket file of the second listener. If the second listener also performs name reclamation, the
/// ensuing deletion will silently fail. Due to the awful design of Unix, this issue cannot be
/// mitigated.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// use interprocess::local_socket::{
/// 	prelude::*,
/// 	Listener, Stream,
/// 	NameTypeSupport, ToFsName, ToNsName,
/// };
/// use std::io::{self, prelude::*, BufReader};
///
/// // Define a function that checks for errors in incoming connections. We'll use this to filter
/// // through connections that fail on initialization for one reason or another.
/// fn handle_error(conn: io::Result<Stream>) -> Option<Stream> {
/// 	match conn {
/// 		Ok(c) => Some(c),
/// 		Err(e) => {
/// 			eprintln!("Incoming connection failed: {e}");
/// 			None
/// 		}
/// 	}
/// }
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
/// 		// TODO update this
/// 		// One important problem that is easy to handle improperly (or not at all) is the
/// 		// "corpse sockets" that are left when a program that uses a file-type socket name
/// 		// terminates its socket server without deleting the file. There's no single strategy
/// 		// for handling this kind of address-already-occupied error. Services that are supposed
/// 		// to only exist as a single instance running on a system should check if another
/// 		// instance is actually running, and if not, delete the socket file. In this example,
/// 		// we leave this up to the user, but in a real application, you usually don't want to do
/// 		// that.
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
/// // Preemptively allocate a sizeable buffer for receiving at a later moment. This size should be
/// // enough and should be easy to find for the allocator. Since we only have one concurrent
/// // client, there's no need to reallocate the buffer repeatedly.
/// let mut buffer = String::with_capacity(128);
///
/// for conn in listener.incoming().filter_map(handle_error) {
/// 	// Wrap the connection into a buffered receiver right away
/// 	// so that we could receive a single line from it.
/// 	let mut conn = BufReader::new(conn);
/// 	println!("Incoming connection!");
///
/// 	// Since our client example sends first, the server should receive a line and only then
/// 	// send a response. Otherwise, because receiving from and sending to a connection cannot be
/// 	// simultaneous without threads or async, we can deadlock the two processes by having both
/// 	// sides wait for the send buffer to be emptied by the other.
/// 	conn.read_line(&mut buffer)?;
///
/// 	// Now that the receive has come through and the client is waiting on the server's send, do
/// 	// it. (`.get_mut()` is to get the sender, `BufReader` doesn't implement a pass-through
/// 	// `Write`.)
/// 	conn.get_mut().write_all(b"Hello from server!n")?;
///
/// 	// Print out the result, getting the newline for free!
/// 	print!("Client answered: {buffer}");
///
/// 	// Let's add an exit condition to shut the server down gracefully.
/// 	if buffer == "stopn" {
/// 		break;
/// 	}
///
/// 	// Clear the buffer so that the next iteration will display new data instead of messages
/// 	// stacking on top of one another.
/// 	buffer.clear();
/// }
/// # io::Result::<()>::Ok(())
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
	fn accept(&self) -> io::Result<Stream> {
		dispatch!(Self: x in self => x.accept()).map(Stream::from)
	}
	#[inline]
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		dispatch!(Self: x in self => x.set_nonblocking(nonblocking))
	}
	#[inline]
	fn do_not_reclaim_name_on_drop(&mut self) {
		dispatch!(Self: x in self => x.do_not_reclaim_name_on_drop())
	}
}

#[cfg(windows)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
impl From<Listener> for OwnedHandle {
	fn from(l: Listener) -> Self {
		match l {
			Listener::NamedPipe(l) => l.into(),
		}
	}
}

#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
impl From<Listener> for OwnedFd {
	fn from(l: Listener) -> Self {
		match l {
			Listener::UdSocket(l) => l.into(),
		}
	}
}
