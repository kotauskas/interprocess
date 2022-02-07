#[cfg(unix)]
use super::super::{close_by_error, handle_fd_error};
use super::{
    imports::*,
    util::{enable_passcred, raw_get_nonblocking, raw_set_nonblocking},
    PathDropGuard, ToUdSocketPath, UdSocketPath, UdStream,
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    iter::FusedIterator,
    mem::{size_of, zeroed},
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
///
/// Sending and receiving ancillary data:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(uds_scm_credentials)] {
/// use interprocess::{
///     unnamed_pipe::{pipe, UnnamedPipeReader},
///     os::unix::udsocket::{UdStreamListener, UdStream, AncillaryData, AncillaryDataBuf},
/// };
/// use std::{
///     io::{self, prelude::*},
///     fs,
///     iter,
///     borrow::Cow,
///     os::unix::io::{FromRawFd, IntoRawFd},
/// };
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
///
/// // Allocate a sufficient buffer for receiving ancillary data.
/// let mut ancillary_buffer = AncillaryDataBuf::owned_with_capacity(
///     AncillaryData::ENCODED_SIZE_OF_CREDENTIALS
///   + AncillaryData::encoded_size_of_file_descriptors(1),
/// );
/// // Prepare valid credentials.
/// let credentials = AncillaryData::credentials();
///
/// for mut connection in listener.incoming()
///     .filter_map(handle_error) {
///     // Create the file descriptor which we will be sending.
///     let (own_fd, fd_to_send) = pipe()?;
///     // Borrow the file descriptor in a slice right away to send it later.
///     let fds = [fd_to_send.into_raw_fd()];
///     let fd_ancillary = AncillaryData::FileDescriptors(
///         Cow::Borrowed(&fds),
///     );
///     
///     connection.send_ancillary(
///         b"File descriptor and credentials from the server!",
///         iter::once(fd_ancillary),
///     )?;
///     
///     // The receive buffer size depends on the situation, but since this example
///     // mirrors the second one from UdSocket, 64 is sufficient.
///     let mut recv_buffer = [0; 64];
///     connection.recv_ancillary(
///         &mut recv_buffer,
///         &mut ancillary_buffer,
///     )?;
///     
///     println!("Client answered: {}", String::from_utf8_lossy(&recv_buffer));
///
///     // Decode the received ancillary data.
///     let (mut file_descriptors, mut cred) = (None, None);
///     for element in ancillary_buffer.decode() {
///         match element {
///             AncillaryData::FileDescriptors(fds) => file_descriptors = Some(fds),
///             AncillaryData::Credentials {pid, uid, gid} => cred = Some((pid, uid, gid)),
///         }
///     }
///     let mut files = Vec::new();
///     if let Some(fds) = file_descriptors {
///         // There is a possibility that zero file descriptors were sent — let's account for that.
///         for fd in fds.iter().copied() {
///             // This is normally unsafe, but since we know that the descriptor is not owned somewhere
///             // else in the current process, it's fine to do this:
///             let file = unsafe {fs::File::from_raw_fd(fd)};
///             files.push(file);
///         }
///     }
///     for mut file in files {
///         file.write_all(b"Hello foreign file descriptor!\n")?;
///     }
///     if let Some(credentials) = cred {
///         println!("Client\tPID: {}", credentials.0);
///         println!(      "\tUID: {}", credentials.1);
///         println!(      "\tGID: {}", credentials.2);
///     }
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
    unsafe fn from_raw_fd_with_dg(fd: c_int, dg: PathDropGuard<'static>) -> Self {
        Self {
            fd: FdOps::new(fd),
            _drop_guard: dg,
        }
    }

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
        Self::_bind(path.to_socket_path()?, false)
    }
    /// Creates a new listener socket at the specified address, remembers the address, and installs a drop guard that will delete the socket file once the socket is dropped.
    ///
    /// See the documentation of [`bind()`](Self::bind).
    pub fn bind_with_drop_guard<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?, true)
    }
    fn _bind(path: UdSocketPath<'_>, keep_drop_guard: bool) -> io::Result<Self> {
        macro_rules! ehndl {
            ($success:ident, $socket:ident) => {
                if !$success {
                    unsafe { return Err(handle_fd_error($socket)) };
                }
            };
        }

        let addr = path.borrow().try_to::<sockaddr_un>()?;

        let socket = {
            let (success, fd) = unsafe {
                let result = libc::socket(AF_UNIX, SOCK_STREAM, 0);
                (result != -1, result)
            };
            if success {
                fd
            } else {
                return Err(io::Error::last_os_error());
            }
        };

        let success = unsafe {
            libc::bind(
                socket,
                // Double cast because you cannot cast a reference to a pointer of arbitrary type
                // but you can cast any narrow pointer to any other narrow pointer
                &addr as *const _ as *const sockaddr,
                size_of::<sockaddr_un>() as u32,
            )
        } != -1;
        ehndl!(success, socket);

        let success = unsafe {
            // FIXME the standard library uses 128 here without an option to change this
            // number, why? If std has solid reasons to do this, remove this notice and
            // document the method's behavior on this matter explicitly; otherwise, add
            // an option to change this value.
            libc::listen(socket, 128)
        } != -1;
        ehndl!(success, socket);

        unsafe { enable_passcred(socket).map_err(close_by_error(socket))? };

        let dg = if keep_drop_guard {
            PathDropGuard {
                path: path.to_owned(),
                enabled: true,
            }
        } else {
            PathDropGuard::dummy()
        };

        Ok(unsafe {
            // SAFETY: we just created the file descriptor, meaning that it's guaranteeed
            // not to be used elsewhere
            Self::from_raw_fd_with_dg(socket, dg)
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
        unsafe { raw_set_nonblocking(self.fd.0, nonblocking) }
    }
    /// Checks whether the socket is currently in nonblocking mode or not.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        unsafe { raw_get_nonblocking(self.fd.0) }
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
        unsafe { Self::from_raw_fd_with_dg(fd, PathDropGuard::dummy()) }
    }
}

/// An infinite iterator over incoming client connections of a [`UdStreamListener`].
///
/// This iterator is created by the [`incoming`] method on [`UdStreamListener`] — see its documentation for more.
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
