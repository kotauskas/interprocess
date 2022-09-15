use {
    super::{super::ToLocalSocketName, LocalSocketStream},
    std::{
        fmt::{self, Debug, Formatter},
        io,
    },
};

#[cfg(feature = "tokio_support")]
impmod! {local_socket::tokio,
    LocalSocketListener as LocalSocketListenerImpl
}
#[cfg(not(feature = "tokio_support"))]
struct LocalSocketListenerImpl;

/// A Tokio-based local socket server, listening for connections.
///
/// # Example
/// - [Basic server](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/server.rs)
pub struct LocalSocketListener {
    inner: LocalSocketListenerImpl,
}
impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self {
            inner: LocalSocketListenerImpl::bind(name)?,
        })
    }
    /// Listens for incoming connections to the socket, asynchronously waiting until a client is connected.
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        Ok(LocalSocketStream {
            inner: self.inner.accept().await?,
        })
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}
// TODO: incoming
