use crate::os::unix::{
    imports::*,
    udsocket::{
        tokio::UdStream, ToUdSocketPath, UdSocketPath, UdStreamListener as SyncUdStreamListener,
    },
};
use std::{convert::TryFrom, io};

/// A Tokio-based Unix domain byte stream socket server, listening for connections.
///
/// All such sockets have the `SOCK_STREAM` socket type; in other words, this is the Unix domain version of a TCP server.
///
/// Can be freely converted to and from its Tokio counterpart.
///
/// # Examples
/// - [Basic server](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_udstream/server.rs)
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
