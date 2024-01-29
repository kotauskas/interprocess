use super::{LocalSocketStream, ToLocalSocketName};
use std::{io, iter::FusedIterator};

impmod! {local_socket,
    LocalSocketListener as LocalSocketListenerImpl
}

/// A local socket server, listening for connections.
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
/// use interprocess::local_socket::{LocalSocketListener, LocalSocketStream, NameTypeSupport};
/// use std::io::{self, prelude::*, BufReader};
///
/// // Define a function that checks for errors in incoming connections. We'll use this to filter
/// // through connections that fail on initialization for one reason or another.
/// fn handle_error(conn: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
///     match conn {
///         Ok(c) => Some(c),
///         Err(e) => {
///             eprintln!("Incoming connection failed: {e}");
///             None
///         }
///     }
/// }
/// // Pick a name. There isn't a helper function for this, mostly because it's largely unnecessary:
/// // in Rust, `match` is your concise, readable and expressive decision making construct.
/// let name = {
///     // This scoping trick allows us to nicely contain the import inside the `match`, so that if
///     // any imports of variants named `Both` happen down the line, they won't collide with the
///     // enum we're working with here. Maybe someone should make a macro for this.
///     use NameTypeSupport::*;
///     match NameTypeSupport::query() {
///         OnlyPaths => "/tmp/example.sock",
///         OnlyNamespaced | Both => "@example.sock",
///     }
/// };
///
/// // Bind our listener.
/// let listener = match LocalSocketListener::bind(name) {
///     Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
///         // One important problem that is easy to handle improperly (or not at all) is the
///         // "corpse sockets" that are left when a program that uses a file-type socket name
///         // terminates its socket server without deleting the file. There's no single strategy
///         // for handling this kind of address-already-occupied error. Services that are supposed
///         // to only exist as a single instance running on a system should check if another
///         // instance is actually running, and if not, delete the socket file. In this example,
///         // we leave this up to the user, but in a real application, you usually don't want to do
///         // that.
///         eprintln!(
///             "\
///Error: could not start server because the socket file is occupied. Please check if {name} is in \
///use by another process and try again."
///         );
///         return Err(e.into());
///     }
///     x => x?,
/// };
///
/// // The syncronization between the server and client, if any is used, goes here.
/// eprintln!("Server running at {name}");
///
/// // Preemptively allocate a sizeable buffer for receiving at a later moment. This size should be
/// // enough and should be easy to find for the allocator. Since we only have one concurrent
/// // client, there's no need to reallocate the buffer repeatedly.
/// let mut buffer = String::with_capacity(128);
///
/// for conn in listener.incoming().filter_map(handle_error) {
///     // Wrap the connection into a buffered receiver right away
///     // so that we could receive a single line from it.
///     let mut conn = BufReader::new(conn);
///     println!("Incoming connection!");
///
///     // Since our client example sends first, the server should receive a line and only then
///     // send a response. Otherwise, because receiving from and sending to a connection cannot be
///     // simultaneous without threads or async, we can deadlock the two processes by having both
///     // sides wait for the send buffer to be emptied by the other.
///     conn.read_line(&mut buffer)?;
///
///     // Now that the receive has come through and the client is waiting on the server's send, do
///     // it. (`.get_mut()` is to get the sender, `BufReader` doesn't implement a pass-through
///     // `Write`.)
///     conn.get_mut().write_all(b"Hello from server!\n")?;
///
///     // Print out the result, getting the newline for free!
///     print!("Client answered: {buffer}");
///
///     // Let's add an exit condition to shut the server down gracefully.
///     if buffer == "stop\n" {
///         break;
///     }
///
///     // Clear the buffer so that the next iteration will display new data instead of messages
///     // stacking on top of one another.
///     buffer.clear();
/// }
/// # io::Result::<()>::Ok(())
/// ```
pub struct LocalSocketListener(LocalSocketListenerImpl);
impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        LocalSocketListenerImpl::bind(name.to_local_socket_name()?, true).map(Self)
    }
    /// Like [`bind()`](Self::bind) followed by
    /// [`.do_not_reclaim_name_on_drop()`](Self::do_not_reclaim_name_on_drop), but avoids a memory
    /// allocation.
    pub fn bind_without_name_reclamation<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        LocalSocketListenerImpl::bind(name.to_local_socket_name()?, false).map(Self)
    }

    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`.incoming()`](Self::incoming) for a convenient way to create a main loop for a server.
    #[inline]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        self.0.accept().map(LocalSocketStream)
    }
    /// Creates an infinite iterator which calls [`.accept()`](Self::accept) with each iteration.
    /// Used together with `for` loops to conveniently create a main loop for a socket server.
    #[inline]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming::from(self)
    }

    /// Enables or disables the nonblocking mode for the listener. By default, it is disabled.
    ///
    /// In nonblocking mode, calling [`.accept()`] and iterating through [`.incoming()`] will
    /// immediately return a [`WouldBlock`](io::ErrorKind::WouldBlock) error if there is no client
    /// attempting to connect at the moment instead of blocking until one arrives.
    ///
    /// # Platform-specific behavior
    /// ## Windows
    /// The nonblocking mode will be also be set for all new streams produced by [`.accept()`] and
    /// [`.incoming()`].
    ///
    /// [`.accept()`]: Self::accept
    /// [`.incoming()`]: Self::incoming
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Disables [name reclamation](#name-reclamation) on the listener.
    #[inline]
    pub fn do_not_reclaim_name_on_drop(&mut self) {
        self.0.do_not_reclaim_name_on_drop();
    }
}
multimacro! {
    LocalSocketListener,
    forward_debug,
    forward_into_handle,
    forward_as_handle(unix),
    forward_from_handle(unix),
    derive_raw(unix),
}

/// An infinite iterator over incoming client connections of a [`LocalSocketListener`].
///
/// This iterator is created by the [`incoming()`](LocalSocketListener::incoming) method on
/// [`LocalSocketListener`] – see its documentation for more.
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a LocalSocketListener,
}
impl<'a> From<&'a LocalSocketListener> for Incoming<'a> {
    fn from(listener: &'a LocalSocketListener) -> Self {
        Self { listener }
    }
}
impl Iterator for Incoming<'_> {
    type Item = io::Result<LocalSocketStream>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
impl FusedIterator for Incoming<'_> {}
