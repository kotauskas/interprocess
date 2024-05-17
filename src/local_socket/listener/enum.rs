use super::{options::ListenerOptions, r#trait};
use crate::local_socket::{ListenerNonblockingMode, Stream};
#[cfg(unix)]
use crate::os::unix::uds_local_socket as uds_impl;
#[cfg(windows)]
use crate::os::windows::named_pipe::local_socket as np_impl;
use std::{io, iter::FusedIterator};

impmod! {local_socket::dispatch_sync as dispatch}

mkenum!(
/// Local socket server, listening for connections.
///
/// This struct is created by [`ListenerOptions`](super::options::ListenerOptions).
///
/// # Name reclamation
/// *This section only applies to Unix domain sockets.*
///
/// When a Unix domain socket listener is closed, its associated socket file is not automatically
/// deleted. Instead, it remains on the filesystem in a zombie state, neither accepting connections
/// nor allowing a new listener to reuse it – [`create_sync()`] will return
/// [`AddrInUse`](io::ErrorKind::AddrInUse) unless it is deleted manually.
///
/// Interprocess implements *automatic name reclamation* via: when the local socket listener is
/// dropped, it performs [`std::fs::remove_file()`] (i.e. `unlink()`) with the path that was
/// originally passed to [`create_sync()`], allowing for subsequent reuse of the local socket name.
///
/// If the program crashes in a way that doesn't unwind the stack, the deletion will not occur and
/// the socket file will linger on the filesystem, in which case manual deletion will be necessary.
/// Identially, the automatic name reclamation mechanism can be opted out of via
/// [`.do_not_reclaim_name_on_drop()`](trait::Listener::do_not_reclaim_name_on_drop) on the listener
/// or [`.reclaim_name(false)`](super::options::ListenerOptions::reclaim_name) on the builder.
///
/// Note that the socket file can be unlinked by other programs at any time, retaining the inode the
/// listener is bound to but making it inaccessible to peers if it was at its last hardlink. If that
/// happens and another listener takes the same path before the first one performs name reclamation,
/// the socket file deletion wouldn't correspond to the listener being closed, instead deleting the
/// socket file of the second listener. If the second listener also performs name reclamation, the
/// ensuing deletion will silently fail. Due to the awful design of Unix, this issue cannot be
/// mitigated.
///
/// [`create_sync()`]: super::options::ListenerOptions::create_sync
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// use interprocess::local_socket::{
/// 	prelude::*,
/// 	Listener, ListenerOptions, Stream,
/// 	GenericFilePath, GenericNamespaced,
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
///
/// // Pick a name.
/// let printname = "example.sock";
/// let name = printname.to_ns_name::<GenericNamespaced>()?;
///
/// // Configure our listener...
/// let opts = ListenerOptions::new()
/// 	.name(name);
///
/// // ...then create it.
/// let listener = match opts.create_sync() {
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
/// 	conn.get_mut().write_all(b"Hello from server!\n")?;
///
/// 	// Print out the result, getting the newline for free!
/// 	print!("Client answered: {buffer}");
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
	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		dispatch::from_options(options)
	}
	#[inline]
	fn accept(&self) -> io::Result<Stream> {
		dispatch!(Self: x in self => x.accept()).map(Stream::from)
	}
	#[inline]
	fn set_nonblocking(&self, nonblocking: ListenerNonblockingMode) -> io::Result<()> {
		dispatch!(Self: x in self => x.set_nonblocking(nonblocking))
	}
	#[inline]
	fn do_not_reclaim_name_on_drop(&mut self) {
		dispatch!(Self: x in self => x.do_not_reclaim_name_on_drop())
	}
}
impl Iterator for Listener {
	type Item = io::Result<Stream>;
	#[inline(always)]
	fn next(&mut self) -> Option<Self::Item> {
		Some(r#trait::Listener::accept(self))
	}
}
impl FusedIterator for Listener {}
