use {
    super::{LocalSocketStream, ToLocalSocketName},
    std::{
        fmt::{self, Debug, Formatter},
        io,
        iter::FusedIterator,
    },
};

impmod! {local_socket,
    LocalSocketListener as LocalSocketListenerImpl
}

/// A local socket server, listening for connections.
///
/// # Examples
/// - [Basic server](https://github.com/kotauskas/interprocess/blob/main/examples/local_socket/server.rs)
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
    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`incoming`] for a convenient way to create a main loop for a server.
    ///
    /// [`incoming`]: #method.incoming " "
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        Ok(LocalSocketStream {
            inner: self.inner.accept()?,
        })
    }
    /// Creates an infinite iterator which calls `accept()` with each iteration. Used together with `for` loops to conveniently create a main loop for a socket server.
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming::from(self)
    }
    /// Enables or disables the nonblocking mode for the listener. By default, it is disabled.
    ///
    /// In nonblocking mode, calling [`accept`] and iterating through [`incoming`] will immediately return a [`WouldBlock`] error if there is no client attempting to connect at the moment instead of blocking until one arrives.
    ///
    /// # Platform-specific behavior
    /// ## Windows
    /// The nonblocking mode will be also be set for the streams produced by [`accept`] and [`incoming`], both existing and new ones.
    ///
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
    /// [`accept`]: #method.accept " "
    /// [`incoming`]: #method.incoming " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

/// An infinite iterator over incoming client connections of a [`LocalSocketListener`].
///
/// This iterator is created by the [`incoming`] method on [`LocalSocketListener`] – see its documentation for more.
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
/// [`incoming`]: struct.LocalSocketListener.html#method.incoming " "
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
