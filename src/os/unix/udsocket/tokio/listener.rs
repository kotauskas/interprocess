use crate::os::unix::udsocket::{
    tokio::UdStream, ToUdSocketPath, UdSocketPath, UdStreamListener as SyncUdStreamListener,
};
use std::{convert::TryFrom, io, os::unix::net::UnixListener as StdUdStreamListener};
use tokio::net::UnixListener as TokioUdStreamListener;

/// A Tokio-based Unix domain byte stream socket server, listening for connections.
///
/// All such sockets have the `SOCK_STREAM` socket type; in other words, this is the Unix domain version of a TCP server.
///
/// Can be freely converted to and from its Tokio counterpart.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// use interprocess::os::unix::udsocket::tokio::{UdStream, UdStreamListener};
/// use std::io;
/// use tokio::{
///     io::{AsyncReadExt, AsyncWriteExt},
///     sync::oneshot::Sender,
///     try_join,
/// };
///
/// // Describe the things we do when we've got a connection ready.
/// async fn handle_conn(mut conn: UdStream) -> io::Result<()> {
///     // Split the connection into two halves to process
///     // received and sent data concurrently.
///     let (mut reader, mut writer) = conn.split();
///
///     // Allocate a sizeable buffer for reading.
///     // This size should be enough and should be easy to find for the allocator.
///     let mut buffer = String::with_capacity(128);
///
///     // Describe the write operation as first writing our whole message, and
///     // then shutting down the write half to send an EOF to help the other
///     // side determine the end of the transmission.
///     let write = async {
///         writer.write_all(b"Hello from server!").await?;
///         writer.shutdown()?;
///         Ok(())
///     };
///
///     // Describe the read operation as reading into our big buffer.
///     let read = reader.read_to_string(&mut buffer);
///
///     // Run both the write-and-send-EOF operation and the read operation concurrently.
///     try_join!(read, write)?;
///
///     // Dispose of our connection right now and not a moment later because I want to!
///     drop(conn);
///
///     // Produce our output!
///     println!("Client answered: {}", buffer.trim());
///     Ok(())
/// }
///
/// static SOCKET_PATH: &str = "/tmp/example.sock";
///
/// // Create our listener. In a more robust program, we'd check for an
/// // existing socket file that has not been deleted for whatever reason,
/// // ensure it's a socket file and not a normal file, and delete it.
/// let listener = UdStreamListener::bind(SOCKET_PATH)?;
/// // This is the part where you tell clients that the server is up,
/// // if you intend to do that at all.
/// println!("Server running at {SOCKET_PATH}");
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
///             eprintln!("error while handling connection: {e}");
///         }
///     });
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct UdStreamListener(TokioUdStreamListener);
impl UdStreamListener {
    /// Creates a new listener socket at the specified address.
    ///
    /// If the socket path exceeds the [maximum socket path length] (which includes the first 0 byte when using the [socket namespace]), an error is returned. Errors can also be produced for different reasons, i.e. errors should always be handled regardless of whether the path is known to be short enough or not.
    ///
    /// # Example
    /// See [`ToUdSocketPath`].
    ///
    /// # System calls
    /// - `socket`
    /// - `bind`
    ///
    /// [maximum socket path length]: super::super::MAX_UDSOCKET_PATH_LEN
    /// [socket namespace]: super::super::UdSocketPath::Namespaced
    pub fn bind<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?)
    }
    fn _bind(path: UdSocketPath<'_>) -> io::Result<Self> {
        let listener = SyncUdStreamListener::_bind(path, false, true)?;
        Self::from_sync(listener)
    }
    /// Listens for incoming connections to the socket, asynchronously waiting a client is connected.
    pub async fn accept(&self) -> io::Result<UdStream> {
        Ok(self.0.accept().await?.0.into())
    }
    tokio_wrapper_conversion_methods!(
        sync SyncUdStreamListener,
        std StdUdStreamListener,
        tokio TokioUdStreamListener);
}
tokio_wrapper_trait_impls!(
    for UdStreamListener,
    sync SyncUdStreamListener,
    std StdUdStreamListener,
    tokio TokioUdStreamListener);
