#[cfg(uds_supported)]
use super::c_wrappers;
use super::{imports::*, PathDropGuard, ToUdSocketPath, UdSocketPath, UdStream};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    iter::FusedIterator,
    mem::zeroed,
};
use to_method::To;

/// A Unix domain byte stream socket server, listening for connections.
///
/// All such sockets have the `SOCK_STREAM` socket type; in other words, this is the Unix domain version of a TCP server.
///
/// # Examples
/// Basic server:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(unix)] {
/// use interprocess::os::unix::udsocket::{UdStream, UdStreamListener};
/// use std::{io::{self, prelude::*}, net::Shutdown};
///
/// fn handle_error(result: io::Result<UdStream>) -> Option<UdStream> {
///     match result {
///         Ok(val) => Some(val),
///         Err(error) => {
///             eprintln!("There was an error with an incoming connection: {}", error);
///             None
///         }
///     }
/// }
///
/// let listener = UdStreamListener::bind("/tmp/example.sock")?;
/// // Outside the loop so that we could reuse the memory allocation for every client
/// let mut input_string = String::new();
/// for mut conn in listener.incoming()
///     // Use filter_map to report all errors with connections and skip those connections in the loop,
///     // making the actual server loop part much cleaner than if it contained error handling as well.
///     .filter_map(handle_error) {
///     conn.write_all(b"Hello from server!")?;
///     conn.shutdown(Shutdown::Write)?;
///     conn.read_to_string(&mut input_string)?;
///     println!("Client answered: {}", input_string);
///     input_string.clear();
/// }
/// # }
/// # Ok(()) }
/// ```
pub struct UdStreamListener {
    // TODO make this not 'static
    _drop_guard: PathDropGuard<'static>,
    fd: FdOps,
}
impl UdStreamListener {
    /// Creates a new listener socket at the specified address.
    ///
    /// If the socket path exceeds the [maximum socket path length] (which includes the first 0 byte when using the [socket namespace]), an error is returned. Errors can also be produced for different reasons, i.e. errors should always be handled regardless of whether the path is known to be short enough or not.
    ///
    /// After the socket is dropped, the socket file will be left over. Use [`bind_with_drop_guard()`](Self::bind_with_drop_guard) to mitigate this automatically, even during panics (if unwinding is enabled).
    ///
    /// # Example
    /// See [`ToUdSocketPath`].
    ///
    /// # System calls
    /// - `socket`
    /// - `bind`
    ///
    /// [maximum socket path length]: const.MAX_UDSOCKET_PATH_LEN.html " "
    /// [socket namespace]: enum.UdSocketPath.html#namespaced " "
    /// [`ToUdSocketPath`]: trait.ToUdSocketPath.html " "
    pub fn bind<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?, false, false)
    }
    /// Creates a new listener socket at the specified address, remembers the address, and installs a drop guard that will delete the socket file once the socket is dropped.
    ///
    /// See the documentation of [`bind()`](Self::bind).
    pub fn bind_with_drop_guard<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?, true, false)
    }
    pub(crate) fn _bind(
        path: UdSocketPath<'_>,
        keep_drop_guard: bool,
        nonblocking: bool,
    ) -> io::Result<Self> {
        let addr = path.borrow().try_to::<sockaddr_un>()?;

        let fd = c_wrappers::create_uds(SOCK_STREAM, nonblocking)?;
        unsafe {
            // SAFETY: addr is well-constructed
            c_wrappers::bind(&fd, &addr)?;
        }
        // FIXME the standard library uses 128 here without an option to change this
        // number, why? If std has solid reasons to do this, remove this notice and
        // document the method's behavior on this matter explicitly; otherwise, add
        // an option to change this value.
        c_wrappers::listen(&fd, 128)?;
        c_wrappers::set_passcred(&fd, true)?;

        let dg = if keep_drop_guard {
            PathDropGuard {
                path: path.to_owned(),
                enabled: true,
            }
        } else {
            PathDropGuard::dummy()
        };

        Ok(Self {
            fd,
            _drop_guard: dg,
        })
    }

    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`incoming`] for a convenient way to create a main loop for a server.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # #[cfg(uds_scm_credentials)] {
    /// use interprocess::os::unix::udsocket::UdStreamListener;
    ///
    /// let listener = UdStreamListener::bind("/tmp/example.sock")?;
    /// loop {
    ///     match listener.accept() {
    ///         Ok(connection) => {
    ///             println!("New client!");
    ///         },
    ///         Err(error) => {
    ///             println!("Incoming connection failed: {}", error);
    ///         },
    ///     }
    /// }
    /// # }
    /// # Ok(()) }
    /// ```
    ///
    /// # System calls
    /// - `accept`
    ///
    /// [`incoming`]: #method.incoming " "
    pub fn accept(&self) -> io::Result<UdStream> {
        let (success, fd) = unsafe {
            let result = libc::accept(self.as_raw_fd(), zeroed(), zeroed());
            (result != -1, result)
        };
        if success {
            Ok(unsafe {
                // SAFETY: we just created the file descriptor, meaning that it's guaranteeed
                // not to be used elsewhere
                UdStream::from_raw_fd(fd)
            })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Creates an infinite iterator which calls `accept()` with each iteration. Used together with `for` loops to conveniently create a main loop for a socket server.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # #[cfg(unix)] {
    /// use interprocess::os::unix::udsocket::UdStreamListener;
    ///
    /// let listener = UdStreamListener::bind("/tmp/example.sock")?;
    /// // Thanks to incoming(), you get a simple self-documenting infinite server loop
    /// for connection in listener.incoming()
    ///     .map(|conn| if let Err(error) = conn {
    ///         eprintln!("Incoming connection failed: {}", error);
    ///     }) {
    ///     eprintln!("New client!");
    /// #   drop(connection);
    /// }
    /// # }
    /// # Ok(()) }
    /// ```
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming::from(self)
    }

    /// Enables or disables the nonblocking mode for the listener. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to [`accept`], and, by extension, iteration through [`incoming`] will never wait for a client to become available to connect and will instead return a [`WouldBlock`] error immediately, allowing the thread to perform other useful operations while there are no new client connections to accept.
    ///
    /// [`accept`]: #method.accept " "
    /// [`incoming`]: #method.incoming " "
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        c_wrappers::set_nonblocking(&self.fd, nonblocking)
    }
    /// Checks whether the socket is currently in nonblocking mode or not.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        c_wrappers::get_nonblocking(&self.fd)
    }
}
impl Debug for UdStreamListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdStreamListener")
            .field("fd", &self.as_raw_fd())
            .field("has_drop_guard", &self._drop_guard.enabled)
            .finish()
    }
}
impl AsRawFd for UdStreamListener {
    #[cfg(unix)]
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
impl IntoRawFd for UdStreamListener {
    #[cfg(unix)]
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
impl FromRawFd for UdStreamListener {
    #[cfg(unix)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        let fd = unsafe { FdOps::from_raw_fd(fd) };
        Self {
            fd,
            _drop_guard: PathDropGuard::dummy(),
        }
    }
}

/// An infinite iterator over incoming client connections of a [`UdStreamListener`].
///
/// This iterator is created by the [`incoming`] method on [`UdStreamListener`] – see its documentation for more.
///
/// [`UdStreamListener`]: struct.UdStreamListener.html " "
/// [`incoming`]: struct.UdStreamListener.html#method.incoming " "
pub struct Incoming<'a> {
    listener: &'a UdStreamListener,
}
impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<UdStream>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
impl FusedIterator for Incoming<'_> {}
impl<'a> From<&'a UdStreamListener> for Incoming<'a> {
    fn from(listener: &'a UdStreamListener) -> Self {
        Self { listener }
    }
}
