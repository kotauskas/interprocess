use {
    super::{LocalSocketStream, ToLocalSocketName},
    std::{io, iter::FusedIterator},
};

impmod! {local_socket,
    LocalSocketListener as LocalSocketListenerImpl
}

/// A local socket server, listening for connections.
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
        LocalSocketListenerImpl::bind(name.to_local_socket_name()?).map(Self)
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
    /// The nonblocking mode will be also be set for the streams produced by [`.accept()`] and
    /// [`.incoming()`], both existing and new ones.
    ///
    /// [`.accept()`]: Self::accept
    /// [`.incoming()`]: Self::incoming
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
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
