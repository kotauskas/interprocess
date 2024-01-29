use super::{super::ToLocalSocketName, LocalSocketStream};
use std::io;

impmod! {local_socket::tokio,
    LocalSocketListener as LocalSocketListenerImpl
}

// TODO borrowed split in examples

/// A Tokio-based local socket server, listening for connections.
///
/// [Name reclamation](super::super::LocalSocketStream#name-reclamation) is performed by default on
/// backends that necessitate it.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::local_socket::{
///     tokio::{LocalSocketListener, LocalSocketStream},
///     NameTypeSupport,
/// };
/// use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, try_join};
/// use std::io;
///
/// // Describe the things we do when we've got a connection ready.
/// async fn handle_conn(conn: LocalSocketStream) -> io::Result<()> {
///     // Split the connection into two halves to process
///     // received and sent data separately.
///     let (recver, mut sender) = conn.split();
///     let mut recver = BufReader::new(recver);
///
///     // Allocate a sizeable buffer for receiving.
///     // This size should be big enough and easy to find for the allocator.
///     let mut buffer = String::with_capacity(128);
///
///     // Describe the send operation as sending our whole message.
///     let send = sender.write_all(b"Hello from server!\n");
///     // Describe the receive operation as receiving a line into our big buffer.
///     let recv = recver.read_line(&mut buffer);
///
///     // Run both operations concurrently.
///     try_join!(recv, send)?;
///
///     // Dispose of our connection right now and not a moment later because I want to!
///     drop((recver, sender));
///
///     // Produce our output!
///     println!("Client answered: {}", buffer.trim());
///     Ok(())
/// }
///
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
/// // Create our listener. In a more robust program, we'd check for an
/// // existing socket file that has not been deleted for whatever reason,
/// // ensure it's a socket file and not a normal file, and delete it.
/// let listener = LocalSocketListener::bind(name)?;
///
/// // The syncronization between the server and client, if any is used, goes here.
/// eprintln!("Server running at {name}");
///
/// // Set up our loop boilerplate that processes our incoming connections.
/// loop {
///     // Sort out situations when establishing an incoming connection caused an error.
///     let conn = match listener.accept().await {
///         Ok(c) => c,
///         Err(e) => {
///             eprintln!("There was an error with an incoming connection: {e}");
///             continue;
///         }
///     };
///
///     // Spawn new parallel asynchronous tasks onto the Tokio runtime
///     // and hand the connection over to them so that multiple clients
///     // could be processed simultaneously in a lightweight fashion.
///     tokio::spawn(async move {
///         // The outer match processes errors that happen when we're
///         // connecting to something. The inner if-let processes errors that
///         // happen during the connection.
///         if let Err(e) = handle_conn(conn).await {
///             eprintln!("Error while handling connection: {e}");
///         }
///     });
/// }
/// # Ok(()) }
/// ```
pub struct LocalSocketListener(LocalSocketListenerImpl);
impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    #[inline]
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        LocalSocketListenerImpl::bind(name.to_local_socket_name()?, true).map(Self::from)
    }
    /// Like [`bind()`](Self::bind) followed by
    /// [`.do_not_reclaim_name_on_drop()`](Self::do_not_reclaim_name_on_drop), but avoids a memory
    /// allocation.
    pub fn bind_without_name_reclamation<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        LocalSocketListenerImpl::bind(name.to_local_socket_name()?, false).map(Self)
    }

    /// Listens for incoming connections to the socket, asynchronously waiting until a client is
    /// connected.
    #[inline]
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        Ok(LocalSocketStream(self.0.accept().await?))
    }

    /// Disables [name reclamation](super::super::LocalSocketStream#name-reclamation) on the listener.
    #[inline]
    pub fn do_not_reclaim_name_on_drop(&mut self) {
        self.0.do_not_reclaim_name_on_drop();
    }
}
#[doc(hidden)]
impl From<LocalSocketListenerImpl> for LocalSocketListener {
    #[inline]
    fn from(inner: LocalSocketListenerImpl) -> Self {
        Self(inner)
    }
}
multimacro! {
    LocalSocketListener,
    forward_as_handle(unix),
    forward_try_handle(LocalSocketListenerImpl, unix),
    forward_debug,
    derive_asraw(unix),
}
// TODO: incoming
