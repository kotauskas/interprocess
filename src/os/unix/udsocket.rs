//! Support for Unix domain sockets, abbreviated here as "Ud-sockets".
//!
//! Ud-sockets are a special kind of sockets which work in the scope of only one system and use file paths instead of IPv4/IPv6 addresses and 16-bit socket numbers. Aside from their high reliability and convenience for the purposes of IPC (such as filesystem-level privelege management and the similarity to named pipes), they have a unique feature which cannot be replaced by any other form of IPC: **ancillary data**.
//!
//! # Ancillary data
//! Thanks to this feature, Ud-sockets can transfer ownership of a file descriptor to another process, even if it doesn't have a parent-child relationship with the file descriptor owner and thus does not inherit anything via `fork()`. Aside from that, ancillary data can contain credentials of a process, which are validated by the kernel unless the sender is the superuser, meaning that this way of retrieving credentials can be used for authentification.
//!
//! # Usage
//! The [`UdStreamListener`] and [`UdSocket`] types are two starting points, depending on whether you intend to use UDP-like datagrams or TCP-like byte streams.
//!
//! [`UdStreamListener`]: struct.UdStreamListener.html " "
//! [`UdSocket`]: struct.UdSocket.html " "

#![cfg_attr(any(not(unix), doc), allow(unused_imports))]

use cfg_if::cfg_if;
use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
    ffi::{CStr, CString, NulError, OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io::{self, IoSlice, IoSliceMut, Read, Write},
    iter::{self, FromIterator, FusedIterator},
    mem::{self, zeroed},
    path::{Path, PathBuf},
    ptr,
};

use super::imports::*;

#[allow(unused_imports)]
use crate::{ReliableReadMsg, Sealed};

#[cfg(unix)]
cfg_if! {
    if #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "emscripten",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "hermit",
        target_os = "redox",
        // For some unknown reason, Newlib only declares sockaddr_un on Xtensa
        all(target_env = "newlib", target_arch = "xtensa"),
        target_env = "uclibc",
    ))] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 108;
    } else if #[cfg(any( // why are those a thing
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
        target_os = "macos",
        target_os = "ios",
    ))] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 104;
    } else {
        compile_error("\
Please fill out MAX_UDSOCKET_PATH_LEN in interprocess/src/os/unix/udsocket.rs for your platform \
if you wish to enable Unix domain socket support for it"
        )
    }
}

/// The maximum path length for Unix domain sockets. [`UdStreamListener::bind`] panics if the specified path exceeds this value.
///
/// When using the [socket namespace], this value is reduced by 1, since enabling the usage of that namespace takes up one character.
///
/// ## Value
/// The following platforms define the value of this constant as **108**:
/// - Linux
///     - includes Android
/// - uClibc
/// - Newlib
///     - *Only supported on Xtensa*
/// - Emscripten
/// - Redox
/// - HermitCore
/// - Solaris
/// - Illumos
///
/// The following platforms define the value of this constant as **104**:
/// - FreeBSD
/// - OpenBSD
/// - NetBSD
/// - DragonflyBSD
/// - macOS
/// - iOS
///
/// [`UdStreamListener::bind`]: struct.UdStreamListener.html#method.bind " "
/// [socket namespace]: enum.UdSocketPath.html#namespaced " "
// The reason why this constant wraps the underscored one instead of being defined directly is
// because that'd require documenting both branches separately. This way, the user-faced
// constant has only one definition and one documentation comment block.
pub const MAX_UDSOCKET_PATH_LEN: usize = _MAX_UDSOCKET_PATH_LEN;

#[inline]
#[allow(unused_variables)]
unsafe fn enable_passcred(socket: i32) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let passcred: c_int = 1;
        libc::setsockopt(
            socket,
            SOL_SOCKET,
            SO_PASSCRED,
            &passcred as *const _ as *const _,
            mem::size_of_val(&passcred) as u32,
        ) != -1
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        true
    } // Cannot have passcred on macOS and iOS.
}

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
/// use std::io::{self, prelude::*};
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
/// for mut connection in listener.incoming()
///     // Use filter_map to report all errors with connections and skip those connections in the loop,
///     // making the actual server loop part much cleaner than if it contained error handling as well.
///     .filter_map(handle_error) {
///     connection.write_all(b"Hello from server!");
///     let mut input_string = String::new();
///     connection.read_to_string(&mut input_string);
///     println!("Client answered: {}", input_string);
/// }
/// # }
/// # Ok(()) }
/// ```
///
/// Sending and receiving ancillary data:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))] {
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
///     );
///     
///     // The receive buffer size depends on the situation, but since this example
///     // mirrors the second one from UdSocket, 64 is sufficient.
///     let mut recv_buffer = [0; 64];
///     connection.recv_ancillary(
///         &mut recv_buffer,
///         &mut ancillary_buffer,
///     );
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
///         file.write(b"Hello foreign file descriptor!");
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
    fd: FdOps,
}
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
    /// [maximum socket path length]: const.MAX_UDSOCKET_PATH_LEN.html " "
    /// [socket namespace]: enum.UdSocketPath.html#namespaced " "
    /// [`ToUdSocketPath`]: trait.ToUdSocketPath.html " "
    pub fn bind<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let path = path.to_socket_path()?; // Shadow original by conversion
        let (addr, addrlen) = unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            (addr, mem::size_of::<sockaddr_un>())
        };
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
            // If binding didn't fail, start listening and return true if it succeeded and false if
            // it failed; if binding failed, short-circuit to returning false
            if libc::bind(
                socket,
                // Double cast because you cannot cast a reference to a pointer of arbitrary type
                // but you can cast any narrow pointer to any other narrow pointer
                &addr as *const _ as *const _,
                addrlen as u32,
            ) != -1
            // FIXME the standard library uses 128 here without an option to change this
            // number, why? If std has solid reasons to do this, remove this notice and
            // document the method's behavior on this matter explicitly; otherwise, add
            // an option to change this value.
            && libc::listen(socket, 128) != -1
            {
                enable_passcred(socket)
            } else {
                false
            }
        };
        if success {
            Ok(unsafe {
                // SAFETY: we just created the file descriptor, meaning that it's guaranteeed
                // not to be used elsewhere
                Self::from_raw_fd(socket)
            })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`incoming`] for a convenient way to create a main loop for a server.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))] {
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
    #[inline]
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
    /// }
    /// # }
    /// # Ok(()) }
    /// ```
    #[inline(always)]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming::from(self)
    }
}
impl Debug for UdStreamListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdStreamListener")
            .field("file_descriptor", &self.as_raw_fd())
            .finish()
    }
}
#[cfg(unix)]
impl AsRawFd for UdStreamListener {
    #[inline(always)]
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
#[cfg(unix)]
impl IntoRawFd for UdStreamListener {
    #[inline(always)]
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for UdStreamListener {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps(fd) }
    }
}

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`] or by connecting to an existing server.
///
/// # Examples
/// Basic example:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(unix)] {
/// use interprocess::os::unix::udsocket::UdStream;
/// use std::io::prelude::*;
///
/// let mut conn = UdStream::connect("/tmp/example1.sock")?;
/// conn.write(b"Hello from client!");
/// let mut string_buffer = String::new();
/// conn.read_to_string(&mut string_buffer);
/// println!("Server answered: {}", string_buffer);
/// # }
/// # Ok(()) }
/// ```
///
/// Receiving and sending ancillary data:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))] {
/// use interprocess::os::unix::udsocket::{UdStream, AncillaryData, AncillaryDataBuf};
/// use std::{
///     io::{self, prelude::*},
///     borrow::Cow,
///     fs,
///     os::unix::io::{IntoRawFd, FromRawFd},
/// };
///
/// // Create one file descriptor which we will be sending.
/// let fd = fs::File::open("/tmp/example_file.mfa")?.into_raw_fd();
/// // Borrow the file descriptor in a slice right away to send it later.
/// let fds = [fd];
/// let fd_ancillary = AncillaryData::FileDescriptors(
///     Cow::Borrowed(&fds),
/// );
/// // Prepare valid credentials. Keep in mind that this is not supported on Apple platforms.
/// let credentials = AncillaryData::credentials();
/// // Allocate a sufficient buffer for receiving ancillary data.
/// let mut ancillary_buffer = AncillaryDataBuf::owned_with_capacity(
///     AncillaryData::ENCODED_SIZE_OF_CREDENTIALS
///   + AncillaryData::encoded_size_of_file_descriptors(1),
/// );
///
/// let conn = UdStream::connect("/tmp/example2.sock")?;
///
/// conn.send_ancillary(
///     b"File descriptor and credentials from client!",
///     [fd_ancillary, credentials].iter().map(|x| x.clone_ref()),
/// )?;
/// // The receive buffer size depends on the situation, but since this example
/// // mirrors the second one from UdSocketListener, 64 is sufficient.
/// let mut recv_buffer = [0; 64];
///
/// conn.recv_ancillary(
///     &mut recv_buffer,
///     &mut ancillary_buffer,
/// )?;
/// println!("Server answered: {}", String::from_utf8_lossy(&recv_buffer));
/// // Decode the received ancillary data.
/// let (mut file_descriptors, mut cred) = (None, None);
/// for element in ancillary_buffer.decode() {
///     match element {
///         AncillaryData::FileDescriptors(fds) => file_descriptors = Some(fds),
///         AncillaryData::Credentials {pid, uid, gid} => cred = Some((pid, uid, gid)),
///     }
/// }
/// let mut files = Vec::new();
/// if let Some(fds) = file_descriptors {
///     // There is a possibility that zero file descriptors were sent — let's account for that.
///     for fd in fds.iter().copied() {
///         // This is normally unsafe, but since we know that the descriptor is not owned somewhere
///         // else in the current process, it's fine to do this:
///         let file = unsafe {fs::File::from_raw_fd(fd)};
///         files.push(file);
///     }
/// }
/// for mut file in files {
///     file.write(b"Hello foreign file descriptor!");
/// }
/// if let Some(credentials) = cred {
///     println!("Server\tPID: {}", credentials.0);
///     println!(      "\tUID: {}", credentials.1);
///     println!(      "\tGID: {}", credentials.2);
/// }
/// # }
/// # Ok(()) }
/// ```
///
/// [`UdStreamListener`]: struct.UdStreamListener.html " "
pub struct UdStream {
    fd: FdOps,
}
impl UdStream {
    /// Connect to a Unix domain socket server at the specified path.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # #[cfg(unix)] {
    /// use interprocess::os::unix::udsocket::UdStream;
    ///
    /// let conn = UdStream::connect("/tmp/example.sock")?;
    /// // Handle the connection to the server
    /// # }
    /// # Ok(()) }
    /// ```
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `socket`
    /// - `connect`
    ///
    /// [`ToUdSocketPath`]: trait.ToUdSocketPath.html " "
    #[inline]
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let path = path.to_socket_path()?; // Shadow original by conversion
        let (addr, addrlen) = unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            (addr, mem::size_of::<sockaddr_un>())
        };
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
            if libc::connect(
                socket,
                // Same as in UdSocketListener::bind()
                &addr as *const _ as *const _,
                addrlen as u32,
            ) != -1
            {
                enable_passcred(socket)
            } else {
                false
            }
        };
        if success {
            Ok(unsafe { Self::from_raw_fd(socket) })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Receives bytes from the socket stream.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions may use `read` instead; for now, this method is a wrapper around [`recv_vectored`].
    ///
    /// [`recv_vectored`]: #method.recv_vectored " "
    // TODO use read
    #[inline(always)]
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.fd.read(buf)
    }
    /// Receives bytes from the socket stream, making use of [scatter input] for the main data.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions may use `readv` instead; for now, this method is a wrapper around [`recv_ancillary_vectored`].
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    /// [`recv_ancillary_vectored`]: #method.recv_ancillary_vectored " "
    // TODO use readv
    #[inline(always)]
    pub fn recv_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut abuf = AncillaryDataBuf::Owned(Vec::new());
        self.recv_ancillary_vectored(bufs, &mut abuf).map(|x| x.0)
    }
    /// Receives both bytes and ancillary data from the socket stream.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, mutable slices of bytes (`u8` values) can be passed directly.
    ///
    /// # System calls
    /// - `recvmsg`
    #[inline(always)]
    pub fn recv_ancillary<'a: 'b, 'b>(
        &self,
        buf: &mut [u8],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, usize)> {
        self.recv_ancillary_vectored(&[IoSliceMut::new(buf)], abuf)
    }
    /// Receives bytes and ancillary data from the socket stream, making use of [scatter input] for the main data.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, mutable slices of bytes (`u8` values) can be passed directly.
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[inline]
    #[allow(clippy::useless_conversion)]
    pub fn recv_ancillary_vectored<'a: 'b, 'b>(
        &self,
        bufs: &[IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, usize)> {
        let abuf: &mut [u8] = abuf.as_mut();
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        hdr.msg_iov = bufs.as_ptr() as *mut _;
        hdr.msg_iovlen = bufs.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "receive buffer array length overflowed `socklen_t`",
            )
        })?;
        hdr.msg_control = abuf.as_mut_ptr() as *mut _;
        hdr.msg_controllen = abuf.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ancillary data receive buffer length overflowed `socklen_t`",
            )
        })?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, mem::transmute::<isize, usize>(result))
        };
        if success {
            Ok((bytes_read, hdr.msg_controllen as _))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Sends bytes into the socket stream.
    ///
    /// # System calls
    /// - `sendmsg`
    ///     - Future versions of `interprocess` may use `write` instead; for now, this method is a wrapper around [`send_vectored`].
    ///
    /// [`send_vectored`]: #method.send_vectored " "
    // TODO use write
    #[inline(always)]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.fd.write(buf)
    }
    /// Sends bytes into the socket stream, making use of [gather output] for the main data.
    ///
    /// # System calls
    /// - `sendmsg`
    ///     - Future versions of `interprocess` may use `writev` instead; for now, this method is a wrapper around [`send_ancillary_vectored`].
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    /// [`send_ancillary_vectored`]: #method.send_ancillary_vectored " "
    // TODO use writev
    #[inline(always)]
    pub fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_ancillary_vectored(bufs, iter::empty())
            .map(|x| x.0)
    }
    /// Sends bytes and ancillary data into the socket stream.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    #[inline(always)]
    pub fn send_ancillary<'a>(
        &self,
        buf: &[u8],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        self.send_ancillary_vectored(&[IoSlice::new(buf)], ancillary_data)
    }

    /// Sends bytes and ancillary data into the socket stream, making use of [gather output] for the main data.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[inline]
    #[allow(clippy::useless_conversion)]
    pub fn send_ancillary_vectored<'a>(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        let abuf_value = ancillary_data
            .into_iter()
            .collect::<EncodedAncillaryData<'_>>();
        let abuf: &[u8] = abuf_value.as_ref();
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        hdr.msg_iov = bufs.as_ptr() as *mut _;
        hdr.msg_iovlen = bufs.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "send buffer array length overflowed `socklen_t`",
            )
        })?;
        hdr.msg_control = abuf.as_ptr() as *mut _;
        hdr.msg_controllen = abuf.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ancillary data send buffer length overflowed `socklen_t`",
            )
        })?;
        let (success, bytes_written) = unsafe {
            let result = libc::sendmsg(self.as_raw_fd(), &hdr as *const _, 0);
            (result != -1, mem::transmute::<isize, usize>(result))
        };
        if success {
            Ok((bytes_written, hdr.msg_controllen as _))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
impl Read for UdStream {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.fd.read(buf)
    }
    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut abuf = AncillaryDataBuf::Owned(Vec::new());
        self.recv_ancillary_vectored(bufs, &mut abuf).map(|x| x.0)
    }
}
impl Write for UdStream {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.fd.write(buf)
    }
    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_ancillary_vectored(bufs, iter::empty())
            .map(|x| x.0)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        // You cannot flush a socket
        Ok(())
    }
}
impl Debug for UdStream {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdStream")
            .field("file_descriptor", &self.as_raw_fd())
            .finish()
    }
}
#[cfg(unix)]
impl AsRawFd for UdStream {
    #[inline(always)]
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
#[cfg(unix)]
impl IntoRawFd for UdStream {
    #[inline(always)]
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for UdStream {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps(fd) }
    }
}

/// A datagram socket in the Unix domain.
///
/// All such sockets have the `SOCK_DGRAM` socket type; in other words, this is the Unix domain version of a UDP socket.
pub struct UdSocket {
    fd: FdOps,
}
impl UdSocket {
    /// Creates a new server socket at the specified address.
    ///
    /// If the socket path exceeds the [maximum socket path length] (which includes the first 0 byte when using the [socket namespace]), an error is returned. Errors can also be produced for different reasons, i.e. errors should always be handled regardless of whether the path is known to be short enough or not.
    ///
    /// # Example
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `socket`
    /// - `bind`
    ///
    /// [maximum socket path length]: const.MAX_UDSOCKET_PATH_LEN.html " "
    /// [socket namespace]: enum.UdSocketPath.html#namespaced " "
    /// [`ToUdSocketPath`]: trait.ToUdSocketPath.html " "
    pub fn bind<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let path = path.to_socket_path()?; // Shadow original by conversion
        let (addr, addrlen) = unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            (addr, mem::size_of::<sockaddr_un>())
        };
        let socket = {
            let (success, fd) = unsafe {
                let result = libc::socket(AF_UNIX, SOCK_DGRAM, 0);
                (result != -1, result)
            };
            if success {
                fd
            } else {
                return Err(io::Error::last_os_error());
            }
        };
        let success = unsafe {
            if libc::bind(
                socket,
                // Double cast because you cannot cast a reference to a pointer of arbitrary type
                // but you can cast any narrow pointer to any other narrow pointer
                &addr as *const _ as *const _,
                addrlen as u32,
            ) != -1
            {
                enable_passcred(socket)
            } else {
                false
            }
        };
        if success {
            Ok(unsafe {
                // SAFETY: we just created the file descriptor, meaning that it's guaranteeed
                // not to be used elsewhere
                Self::from_raw_fd(socket)
            })
        } else {
            Err(io::Error::last_os_error())
        }
    }
    /// Connect to a Unix domain socket server at the specified path.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # #[cfg(unix)] {
    /// use interprocess::os::unix::udsocket::UdSocket;
    ///
    /// let conn = UdSocket::connect("/tmp/example.sock")?;
    /// // Handle the connection to the server
    /// # }
    /// # Ok(()) }
    /// ```
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `socket`
    /// - `connect`
    ///
    /// [`ToUdSocketPath`]: trait.ToUdSocketPath.html " "
    #[inline]
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let path = path.to_socket_path()?; // Shadow original by conversion
        let (addr, addrlen) = unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            (addr, mem::size_of::<sockaddr_un>())
        };
        let socket = {
            let (success, fd) = unsafe {
                let result = libc::socket(AF_UNIX, SOCK_DGRAM, 0);
                (result != -1, result)
            };
            if success {
                fd
            } else {
                return Err(io::Error::last_os_error());
            }
        };
        let success = unsafe {
            if libc::connect(
                socket,
                // Same as in UdSocketListener::bind()
                &addr as *const _ as *const _,
                addrlen as u32,
            ) != -1
            {
                enable_passcred(socket)
            } else {
                false
            }
        };
        if success {
            Ok(unsafe { Self::from_raw_fd(socket) })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Receives a single datagram from the socket, returning how much of the buffer was filled out and whether a part of the datagram was discarded because the buffer was too small.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions of `interprocess` may use `read` instead; for now, this method is a wrapper around [`recv_vectored`].
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    /// [`recv_vectored`]: #method.recv_vectored " "
    // TODO use read
    #[inline(always)]
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, bool)> {
        self.recv_vectored(&mut [IoSliceMut::new(buf)])
    }

    /// Receives a single datagram from the socket, making use of [scatter input] and returning how much of the buffer was filled out and whether a part of the datagram was discarded because the buffer was too small.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions of `interprocess` may use `readv` instead; for now, this method is a wrapper around [`recv_ancillary_vectored`].
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    /// [`recv_ancillary_vectored`]: #method.recv_ancillary_vectored " "
    // TODO use readv
    #[inline(always)]
    pub fn recv_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<(usize, bool)> {
        self.recv_ancillary_vectored(bufs, &mut AncillaryDataBuf::Owned(Vec::new()))
            .map(|x| (x.0, x.1))
    }

    /// Receives a single datagram and ancillary data from the socket. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - Whether a part of the datagram was discarded because the buffer was too small
    /// - How many bytes of ancillary data were received
    /// - Whether some ancillary data was discarded because the buffer was too small
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[inline(always)]
    pub fn recv_ancillary<'a: 'b, 'b>(
        &self,
        buf: &mut [u8],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        self.recv_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf)
    }

    /// Receives a single datagram and ancillary data from the socket, making use of [scatter input]. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - Whether a part of the datagram was discarded because the buffer was too small
    /// - How many bytes of ancillary data were received
    /// - Whether some ancillary data was discarded because the buffer was too small
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[allow(clippy::useless_conversion)]
    pub fn recv_ancillary_vectored<'a: 'b, 'b>(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        let abuf: &mut [u8] = abuf.as_mut();
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        hdr.msg_iov = bufs.as_ptr() as *mut _;
        hdr.msg_iovlen = bufs.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "receive buffer array length overflowed `socklen_t`",
            )
        })?;
        hdr.msg_control = abuf.as_mut_ptr() as *mut _;
        hdr.msg_controllen = abuf.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ancillary data receive buffer length overflowed `socklen_t`",
            )
        })?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, mem::transmute::<isize, usize>(result))
        };
        if success {
            Ok((
                bytes_read,
                hdr.msg_flags & MSG_TRUNC != 0,
                hdr.msg_controllen as _,
                hdr.msg_flags & MSG_CTRUNC != 0,
            ))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Receives a single datagram and the source address from the socket, returning how much of the buffer was filled out and whether a part of the datagram was discarded because the buffer was too small.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions of `interprocess` may use `recvfrom` instead; for now, this method is a wrapper around [`recv_from_vectored`].
    ///
    /// [`recv_from_vectored`]: #method.recv_from_vectored " "
    // TODO use recvfrom
    #[inline(always)]
    pub fn recv_from<'a: 'b, 'b>(
        &self,
        buf: &mut [u8],
        addr_buf: &'b mut UdSocketPath<'a>,
    ) -> io::Result<(usize, bool)> {
        self.recv_from_vectored(&mut [IoSliceMut::new(buf)], addr_buf)
    }

    /// Receives a single datagram and the source address from the socket, making use of [scatter input] and returning how much of the buffer was filled out and whether a part of the datagram was discarded because the buffer was too small.
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[inline(always)]
    pub fn recv_from_vectored<'a: 'b, 'b>(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        addr_buf: &'b mut UdSocketPath<'a>,
    ) -> io::Result<(usize, bool)> {
        self.recv_from_ancillary_vectored(bufs, &mut AncillaryDataBuf::Owned(Vec::new()), addr_buf)
            .map(|x| (x.0, x.1))
    }

    /// Receives a single datagram, ancillary data and the source address from the socket. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - Whether a part of the datagram was discarded because the buffer was too small
    /// - How many bytes of ancillary data were received
    /// - Whether some ancillary data was discarded because the buffer was too small
    ///
    /// # System calls
    /// - `recvmsg`
    #[inline(always)]
    pub fn recv_from_ancillary<'a: 'b, 'b, 'c: 'd, 'd>(
        &self,
        buf: &mut [u8],
        abuf: &'b mut AncillaryDataBuf<'a>,
        addr_buf: &'d mut UdSocketPath<'c>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        self.recv_from_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf, addr_buf)
    }

    /// Receives a single datagram, ancillary data and the source address from the socket, making use of [scatter input]. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - Whether a part of the datagram was discarded because the buffer was too small
    /// - How many bytes of ancillary data were received
    /// - Whether some ancillary data was discarded because the buffer was too small
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[allow(clippy::useless_conversion)]
    pub fn recv_from_ancillary_vectored<'a: 'b, 'b, 'c: 'd, 'd>(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
        addr_buf: &'d mut UdSocketPath<'c>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        let abuf: &mut [u8] = abuf.as_mut();
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        // Same goes for sockaddr_un
        let mut addr_buf_staging = unsafe { zeroed() };
        // It's a void* so the doublecast is mandatory
        hdr.msg_name = &mut addr_buf_staging as *mut _ as *mut _;
        hdr.msg_namelen = mem::size_of_val(&addr_buf_staging) as u32;
        hdr.msg_iov = bufs.as_ptr() as *mut _;
        hdr.msg_iovlen = bufs.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "receive buffer array length overflowed `socklen_t`",
            )
        })?;
        hdr.msg_control = abuf.as_mut_ptr() as *mut _;
        hdr.msg_controllen = abuf.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ancillary data receive buffer length overflowed `socklen_t`",
            )
        })?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, mem::transmute::<isize, usize>(result))
        };
        let path_length = hdr.msg_namelen as usize;
        if success {
            addr_buf.write_sockaddr_un_to_self(&addr_buf_staging, path_length);
            Ok((
                bytes_read,
                hdr.msg_flags & MSG_TRUNC != 0,
                hdr.msg_controllen as _,
                hdr.msg_flags & MSG_CTRUNC != 0,
            ))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Returns the size of the next datagram available on the socket without discarding it.
    ///
    /// This method is only available on Linux since kernel version 2.2. On lower kernel versions, it will fail; on other platforms, it's absent and thus any usage of it will result in a compile-time error.
    ///
    /// # System calls
    /// - `recv`
    #[cfg(any(doc, target_os = "linux"))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
    pub fn peek_msg_size(&self) -> io::Result<usize> {
        let mut buffer = [0_u8; 0];
        let (success, size) = unsafe {
            let size = libc::recv(
                self.as_raw_fd(),
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                MSG_TRUNC | libc::MSG_PEEK,
            );
            (size != -1, mem::transmute::<isize, usize>(size))
        };
        if success {
            Ok(size)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Sends a datagram into the socket.
    ///
    ///
    /// # System calls
    /// - `sendmsg`
    ///     - Future versions of `interprocess` may use `write` instead; for now, this method is a wrapper around [`send_vectored`].
    ///
    /// [`send_vectored`]: #method.send_vectored " "
    #[inline(always)]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.send_vectored(&[IoSlice::new(buf)])
    }
    /// Sends a datagram into the socket, making use of [gather output] for the main data.
    ///
    ///
    /// # System calls
    /// - `sendmsg`
    ///     - Future versions of `interprocess` may use `writev` instead; for now, this method is a wrapper around [`send_ancillary_vectored`].
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    /// [`send_ancillary_vectored`]: #method.send_ancillary_vectored " "
    #[inline(always)]
    pub fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_ancillary_vectored(bufs, iter::empty())
            .map(|x| x.0)
    }
    /// Sends a datagram and ancillary data into the socket.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    #[inline(always)]
    pub fn send_ancillary<'a>(
        &self,
        buf: &[u8],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        self.send_ancillary_vectored(&[IoSlice::new(buf)], ancillary_data)
    }
    /// Sends a datagram and ancillary data into the socket, making use of [gather output] for the main data.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[allow(clippy::useless_conversion)]
    pub fn send_ancillary_vectored<'a>(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        let abuf_value = ancillary_data
            .into_iter()
            .collect::<EncodedAncillaryData<'_>>();
        let abuf: &[u8] = abuf_value.as_ref();
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        hdr.msg_iov = bufs.as_ptr() as *mut _;
        hdr.msg_iovlen = bufs.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "send buffer array length overflowed `socklen_t`",
            )
        })?;
        hdr.msg_control = abuf.as_ptr() as *mut _;
        hdr.msg_controllen = abuf.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ancillary data send buffer length overflowed `socklen_t`",
            )
        })?;
        let (success, bytes_written) = unsafe {
            let result = libc::sendmsg(self.as_raw_fd(), &hdr as *const _, 0);
            (result != -1, mem::transmute::<isize, usize>(result))
        };
        if success {
            Ok((bytes_written, hdr.msg_controllen as _))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
impl Debug for UdSocket {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdSocket")
            .field("file_descriptor", &self.as_raw_fd())
            .finish()
    }
}
#[cfg(target_os = "linux")]
impl ReliableReadMsg for UdSocket {
    #[inline]
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
        let msg_size = self.peek_msg_size()?;
        if msg_size > buf.len() {
            let mut new_buffer = Vec::with_capacity(msg_size);
            self.recv(&mut new_buffer).map(|x| x.0)?;
            Ok(Err(new_buffer))
        } else {
            Ok(Ok(self.recv(buf).map(|x| x.0)?))
        }
    }
    #[inline]
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
        let msg_size = self.peek_msg_size()?;
        if msg_size > buf.len() {
            Ok(Err(msg_size))
        } else {
            Ok(Ok(self.recv(buf).map(|x| x.0)?))
        }
    }
}
#[cfg(unix)]
impl Sealed for UdSocket {}
#[cfg(unix)]
impl AsRawFd for UdSocket {
    #[inline(always)]
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
#[cfg(unix)]
impl IntoRawFd for UdSocket {
    #[inline(always)]
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for UdSocket {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps(fd) }
    }
}

/// Represents a name for a Unix domain socket.
///
/// The main purpose for this enumeration is to conditionally support the dedicated socket namespace on systems which implement it — for that, the `Namespaced` variant is used. Depending on your system, you might not be seeing it, which is when you'd need the `File` fallback variant, which works on all POSIX-compliant systems.
///
/// ## `Namespaced`
/// This variant refers to sockets in a dedicated socket namespace, which is fully isolated from the main filesystem and closes sockets automatically when the server which opened the socket shuts down. **This variant is only implemented on Linux, which is why it is not available on other POSIX-conformant systems at compile time, resulting in a compile-time error if usage is attempted.**
///
/// ## `File`
/// All sockets identified this way are located on the main filesystem and exist as persistent files until deletion, preventing servers from using the same socket without deleting it from the filesystem first. This variant is available on all POSIX-compilant systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UdSocketPath<'a> {
    /// An unnamed socket, identified only by its file descriptor. This is an invalid path value for creating sockets — all attempts to use such a value will result in an error.
    Unnamed,
    /// Identifies a socket which is located in the filesystem tree, existing as a file. See the [enum-level documentation] for more.
    ///
    /// [enum-level documentation]: #file " "
    File(Cow<'a, CStr>),
    /// Identifies a socket in the dedicated socket namespace, where it exists until the server closes it rather than persisting as a file. See the [enum-level documentation] for more.
    ///
    /// [enum-level documentation]: #namespaced " "
    #[cfg(any(target_os = "linux", doc))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
    Namespaced(Cow<'a, CStr>),
}
impl<'a> UdSocketPath<'a> {
    /// Returns the path as a `CStr`. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    #[inline]
    pub fn as_cstr(&'a self) -> &'a CStr {
        match self {
            Self::File(cow) => &*cow,
            #[cfg(any(doc, target_os = "linux"))]
            Self::Namespaced(cow) => &*cow,
            Self::Unnamed => unsafe { CStr::from_bytes_with_nul_unchecked(&[0]) },
        }
    }
    /// Returns the path as a `CString`. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    #[inline]
    pub fn into_cstring(self) -> CString {
        match self {
            Self::File(cow) => cow.into_owned(),
            #[cfg(any(doc, target_os = "linux"))]
            Self::Namespaced(cow) => cow.into_owned(),
            Self::Unnamed => CString::new(Vec::new())
                .unwrap_or_else(|_| unsafe { std::hint::unreachable_unchecked() }),
        }
    }

    /// Ensures that the path is stored as an owned `CString` in place, and returns whether that required cloning or not. If `self` was not referring to any socket ([`Unnamed` variant]), the value is set to an empty `CString` (only nul terminator) of type [`File`].
    ///
    /// [`Unnamed` variant]: #variant.Unnamed " "
    /// [`File`]: #file " "
    #[inline]
    pub fn make_owned(&mut self) -> bool {
        match self {
            Self::File(cow) => match cow {
                Cow::Owned(..) => false,
                Cow::Borrowed(slice) => {
                    *self = Self::File(Cow::Owned(slice.to_owned()));
                    true
                }
            },
            #[cfg(any(doc, target_os = "linux"))]
            Self::Namespaced(cow) => match cow {
                Cow::Owned(..) => false,
                Cow::Borrowed(slice) => {
                    *self = Self::Namespaced(Cow::Owned(slice.to_owned()));
                    true
                }
            },
            Self::Unnamed => {
                *self = Self::File(Cow::Owned(
                    CString::new(Vec::new())
                        .expect("unexpected unrecoverable CString creation error"),
                ));
                true
            }
        }
    }

    /// Returns a mutable reference to the underlying `CString`, cloning the borrowed path if it wasn't owned before.
    #[inline]
    pub fn get_cstring_mut(&mut self) -> &mut CString {
        self.make_owned();
        self.try_get_cstring_mut().unwrap_or_else(|| unsafe {
            // SAFETY: the call to make_owned ensured that there is a CString
            std::hint::unreachable_unchecked()
        })
    }
    /// Returns a mutable reference to the underlying `CString` if it's available as owned, otherwise returns `None`.
    #[inline]
    pub fn try_get_cstring_mut(&mut self) -> Option<&mut CString> {
        let cow = match self {
            Self::File(cow) => cow,
            #[cfg(any(doc, target_os = "linux"))]
            Self::Namespaced(cow) => cow,
            Self::Unnamed => return None,
        };
        match cow {
            Cow::Owned(cstring) => Some(cstring),
            Cow::Borrowed(..) => None,
        }
    }

    /// Returns `true` if the path to the socket is stored as an owned `CString`, i.e. if `into_cstring` doesn't require cloning the path; `false` otherwise.
    #[inline]
    pub fn is_owned(&self) -> bool {
        let cow = match self {
            Self::File(cow) => cow,
            #[cfg(any(doc, target_os = "linux"))]
            Self::Namespaced(cow) => cow,
            Self::Unnamed => return false,
        };
        matches!(cow, Cow::Owned(..))
    }

    #[inline]
    #[cfg(unix)]
    fn write_sockaddr_un_to_self(&mut self, addr: &sockaddr_un, addrlen: usize) {
        let sun_path_length = (addrlen as isize) - (mem::size_of_val(&addr.sun_family) as isize);
        let sun_path_length = match usize::try_from(sun_path_length) {
            Ok(val) => val,
            Err(..) => {
                *self = Self::Unnamed;
                return;
            }
        };
        if let Some(cstring) = self.try_get_cstring_mut() {
            unsafe {
                let mut vec = ptr::read::<CString>(cstring as *const _).into_bytes_with_nul();
                // Write an empty CString to avoid a double-free if a panic happens here, and if it fails, just crash
                ptr::write::<CString>(
                    cstring as *mut _,
                    CString::new(Vec::new()).unwrap_or_else(|_| std::process::abort()),
                );
                #[cfg(any(doc, target_os = "linux"))]
                let (namespaced, src_ptr, path_length) = if addr.sun_path[0] == 0 {
                    (
                        true,
                        addr.sun_path.as_ptr().offset(1) as *const u8,
                        sun_path_length - 1,
                    )
                } else {
                    (false, addr.sun_path.as_ptr() as *const u8, sun_path_length)
                };
                #[cfg(not(any(doc, target_os = "linux")))]
                let (src_ptr, path_length) =
                    { (addr.sun_path.as_ptr() as *const u8, sun_path_length) };
                // Fill the space for the name and the nul terminator with nuls
                vec.resize(path_length, 0);
                ptr::copy_nonoverlapping(src_ptr, vec.as_mut_ptr(), path_length);
                // If the system added a nul byte as part of the length, remove the one we added ourselves.
                if vec.last() == Some(&0) && vec[vec.len() - 2] == 0 {
                    vec.pop();
                }
                // Handle the error anyway — better be safe than sorry
                let new_cstring = match CString::new(vec) {
                    Ok(cstring) => cstring,
                    Err(..) => std::process::abort(),
                };
                #[cfg(any(doc, target_os = "linux"))]
                let path_to_write = if namespaced {
                    UdSocketPath::Namespaced(Cow::Owned(new_cstring))
                } else {
                    UdSocketPath::File(Cow::Owned(new_cstring))
                };
                #[cfg(not(any(doc, target_os = "linux")))]
                let path_to_write = UdSocketPath::File(Cow::Owned(new_cstring));
                let old_val = mem::replace(self, path_to_write);
                // Deallocate the empty CString we wrote in the beginning
                mem::drop(old_val);
            }
        } else {
            #[allow(unused_variables)]
            let (cstring, namespaced) = unsafe {
                let (namespaced, src_ptr, path_length) = if addr.sun_path[0] == 0 {
                    (
                        true,
                        addr.sun_path.as_ptr().offset(1) as *const u8,
                        sun_path_length - 1,
                    )
                } else {
                    (false, addr.sun_path.as_ptr() as *const u8, sun_path_length)
                };
                let mut vec = vec![0; path_length];
                ptr::copy_nonoverlapping(src_ptr, vec.as_mut_ptr(), path_length);
                // If the system added a nul byte as part of the length, remove it.
                if vec.last() == Some(&0) {
                    vec.pop();
                }
                let cstring = match CString::new(vec) {
                    Ok(cstring) => cstring,
                    Err(..) => panic!("unrecoverable memory safety violation threat"),
                };
                (cstring, namespaced)
            };
            #[cfg(any(doc, target_os = "linux"))]
            let path = if namespaced {
                UdSocketPath::Namespaced(Cow::Owned(cstring))
            } else {
                UdSocketPath::File(Cow::Owned(cstring))
            };
            #[cfg(not(any(doc, target_os = "linux")))]
            let path = UdSocketPath::File(Cow::Owned(cstring));
            *self = path;
        }
    }
    /// Returns `addr_len` to pass to `bind`/`connect`.
    #[inline]
    #[cfg(unix)]
    fn write_self_to_sockaddr_un(&self, addr: &mut sockaddr_un) -> io::Result<()> {
        let is_namespaced;
        let len_of_self = self.as_cstr().to_bytes_with_nul().len();
        match self {
            UdSocketPath::File(..) => {
                is_namespaced = false;
                if len_of_self > MAX_UDSOCKET_PATH_LEN {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "socket path should not be longer than {} bytes",
                            MAX_UDSOCKET_PATH_LEN
                        ),
                    ));
                }
            }
            #[cfg(target_os = "linux")]
            UdSocketPath::Namespaced(..) => {
                is_namespaced = true;
                if len_of_self > (MAX_UDSOCKET_PATH_LEN - 1) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "namespaced socket name should not be longer than {} bytes",
                            MAX_UDSOCKET_PATH_LEN - 1
                        ),
                    ));
                }
            }
            UdSocketPath::Unnamed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "must provide a proper name for the socket",
                ))
            }
        }

        unsafe {
            ptr::copy_nonoverlapping(
                self.as_cstr().as_ptr(),
                if is_namespaced {
                    addr.sun_path.as_mut_ptr().offset(1)
                } else {
                    addr.sun_path.as_mut_ptr()
                },
                len_of_self,
            );
        }
        Ok(())
    }
}
impl UdSocketPath<'static> {
    /// Creates a buffer suitable for usage with [`recv_from`] ([`_ancillary`]/[`_vectored`]/[`_ancillary_vectored`]). The capacity is equal to the [`MAX_UDSOCKET_PATH_LEN`] constant (the nul terminator in the `CString` is included). **The contained value is unspecified — results of reading from the buffer should not be relied upon.**
    ///
    /// # Example
    /// ```
    /// # #[cfg(unix)] {
    /// use interprocess::os::unix::udsocket::{UdSocketPath, MAX_UDSOCKET_PATH_LEN};
    /// use std::borrow::Cow;
    ///
    /// let path_buffer = UdSocketPath::buffer();
    /// match path_buffer {
    ///     UdSocketPath::File(cow) => match cow {
    ///         Cow::Owned(cstring)
    ///     => assert_eq!(cstring.into_bytes_with_nul().capacity(), MAX_UDSOCKET_PATH_LEN),
    ///         Cow::Borrowed(..) => unreachable!(),
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// # }
    /// ```
    ///
    /// [`recv_from`]: struct.UdSocket.html#method.recv_from " "
    /// [`_ancillary`]: struct.UdSocket.html#method.recv_from " "
    /// [`_vectored`]: struct.UdSocket.html#method.recv_from_vectored " "
    /// [`_ancillary_vectored`]: struct.UdSocket.html#method.recv_from_ancillary_vectored " "
    /// [`MAX_UDSOCKET_PATH_LEN`]: constant.MAX_UDSOCKET_PATH_LEN.html " "
    #[inline]
    pub fn buffer() -> Self {
        Self::File(Cow::Owned(
            CString::new(vec![0x2F; MAX_UDSOCKET_PATH_LEN - 1])
                .expect("unexpected nul in newly created Vec, possible heap corruption"),
        ))
    }

    /// Constructs a `UdSocketPath::File` value from a `Vec` of bytes, wrapping `CString::new`.
    #[inline]
    pub fn file_from_vec(vec: Vec<u8>) -> Result<Self, NulError> {
        Ok(Self::File(Cow::Owned(CString::new(vec)?)))
    }
    /// Constructs a `UdSocketPath::Namespaced` value from a `Vec` of bytes, wrapping `CString::new`.
    #[cfg(any(target_os = "linux", doc))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
    #[inline]
    pub fn namespaced_from_vec(vec: Vec<u8>) -> Result<Self, NulError> {
        Ok(Self::Namespaced(Cow::Owned(CString::new(vec)?)))
    }
}

/// Trait for types which can be converted to a [path to a Unix domain socket][`UdSocketPath`].
///
/// The difference between this trait and [`TryInto`]`<`[`UdSocketPath`]`>` is that the latter does not constrain the error type to be [`io::Error`] and thus is not compatible with many types from the standard library which are widely expected to be convertible to Unix domain socket paths. Additionally, this makes the special syntax for namespaced sockets possible (see below).
///
/// ## `@` syntax for namespaced paths
/// On Linux (since it's the only platform which supports [namespaced socket paths]), an extra syntax feature is implemented for string types which don't have file path semantics, i.e. all standard string types except for [`Path`] and [`PathBuf`]. If the first character in a string is `@`, the path is interpreted as a namespaced socket path rather than a normal file path. Read the `UdSocketPath` documentation for more on what that means. There are several ways to opt out of that behavior if you're referring to a socket at a relative path which starts from a `@`:
/// - Use [`AsRef`] to convert the string slice type into a [`Path`] which has file path semantics and therefore does not have the `@` syntax enabled, if your string type is [`str`] or [`OsStr`]
/// - Prefix the path with `./`, which carries the same meaning from the perspective of the OS but bypasses the `@` check
/// - If your string type is [`CStr`] or [`CString`], explicitly construct `UdSocketPath`'s `File` variant with a [`Cow`] wrapping your string value
///
/// # Example
/// The following example uses the `UdStreamListener::bind` method, but `UdStream::connect` and `UdSocket::bind`/`UdSocket::connect` accept the same argument types too.
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(unix)] {
/// use interprocess::os::unix::udsocket::{UdStreamListener, UdSocketPath};
/// use std::{ffi::{CStr, CString}, path::{Path, PathBuf}, borrow::Cow};
///
/// // 1. Use a string literal
/// let listener = UdStreamListener::bind("/tmp/example1.sock")?;
/// // If we're on Linux, we can also use the abstract socket namespace which exists separately from
/// // the filesystem thanks to the special @ sign syntax which works with all string types
/// let listener_namespaced = UdStreamListener::bind("@namespaced_socket_1")?;
///
/// // 2. Use an owned string
/// let listener = UdStreamListener::bind("/tmp/example2.sock".to_string())?;
/// // Same story with the namespaced socket here
/// let listener_namespaced = UdStreamListener::bind("@namespaced_socket_2")?;
///
/// // 3. Use a path slice or an owned path
/// let listener_by_path = UdStreamListener::bind(Path::new("/tmp/exmaple3a.sock"))?;
/// let listener_by_pathbuf = UdStreamListener::bind(PathBuf::from("/tmp/example3b.sock"))?;
/// // The @ syntax doesn't work with Path and PathBuf, since those are explicitly paths at the type
/// // level, rather than strings with contextual meaning. Using AsRef to convert an &str slice or
/// // an &OsStr slice into a &Path slice is the recommended way to disable the @ syntax.
///
/// // 4. Use manual creation
/// let cstring = CString::new("/tmp/example4a.sock".to_string().into_bytes())?;
/// let path_to_socket = UdSocketPath::File(Cow::Owned(cstring));
/// let listener = UdStreamListener::bind(path_to_socket);
///
/// let cstr = CStr::from_bytes_with_nul("/tmp/example4b.sock\0".as_bytes())?;
/// let path_to_socket = UdSocketPath::File(Cow::Borrowed(cstr));
/// let listener = UdStreamListener::bind(path_to_socket);
/// # }
/// # Ok(()) }
/// ```
///
/// [`UdSocketPath`]: enum.UdSocketPath.html " "
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html " "
/// [`TryInto`]: https://doc.rust-lang.org/std/convert/trait.TryInto.html " "
/// [`AsRef`]: https://doc.rust-lang.org/std/convert/trait.AsRef.html " "
/// [namespaced socket paths]: struct.UdSocketPath.html#namespaced " "
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
/// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
/// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
/// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
/// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
/// [`Cow`]: https://doc.rust-lang.org/std/borrow/enum.Cow.html " "
/// [`str`]: https://doc.rust-lang.org/stable/std/primitive.str.html
pub trait ToUdSocketPath<'a> {
    /// Performs the conversion from `self` to a Unix domain socket path.
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>>;
}
impl<'a> ToUdSocketPath<'a> for UdSocketPath<'a> {
    /// Accepts explicit `UdSocketPath`s in the `bind` constructor.
    #[inline(always)]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        Ok(self)
    }
}
impl<'a> ToUdSocketPath<'a> for &'a CStr {
    /// Converts a borrowed [`CStr`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        // 0x40 is the ASCII code for @, and since UTF-8 is ASCII-compatible, it would work too
        #[cfg(any(doc, target_os = "linux"))]
        if self.to_bytes().first() == Some(&0x40) {
            let without_at_sign = &self.to_bytes_with_nul()[1..];
            let without_at_sign = unsafe {
                // SAFETY: it's safe to assume that the second byte comes before the nul
                // terminator or is that nul terminator itself if the first one is an @ sign
                CStr::from_bytes_with_nul_unchecked(without_at_sign)
            };
            // Use early return to simplify the conditional inclusion for the @ syntax check.
            return Ok(UdSocketPath::Namespaced(Cow::Borrowed(without_at_sign)));
        }
        Ok(UdSocketPath::File(Cow::Borrowed(self)))
    }
}
impl ToUdSocketPath<'static> for CString {
    /// Converts an owned [`CString`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(any(doc, target_os = "linux"))]
        if self.as_bytes().first() == Some(&0x40) {
            let without_at_sign = {
                let mut without_at_sign = self.into_bytes();
                without_at_sign.remove(0);
                unsafe {
                    // SAFETY: see CStr impl for why this is safe in both impls
                    CString::from_vec_unchecked(without_at_sign)
                }
            };
            // As in the CStr impl, we're using an early return to simplify conditional compilation
            return Ok(UdSocketPath::Namespaced(Cow::Owned(without_at_sign)));
        }
        Ok(UdSocketPath::File(Cow::Owned(self)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a OsStr {
    /// Converts a borrowed [`OsStr`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        #[cfg(any(doc, target_os = "linux"))]
        if self.as_bytes().first() == Some(&0x40) {
            if self.as_bytes().last() != Some(&0) {
                let mut owned = self.to_owned().into_vec();
                owned.remove(0);
                return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(owned)?)));
            } else {
                let without_at_sign = self.as_bytes().split_at(1).0;
                let cstr = CStr::from_bytes_with_nul(without_at_sign)
                    .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
                return Ok(UdSocketPath::Namespaced(Cow::Borrowed(cstr)));
            }
        }
        if self.as_bytes().last() != Some(&0) {
            Ok(UdSocketPath::File(Cow::Owned(CString::new(
                self.to_owned().into_vec(),
            )?)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for OsString {
    /// Converts a borrowed [`OsString`] to an owned `UdSocketPath`. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`OsString`]: https://doc.rust-lang.org/std/ffi/struct.OsString.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(any(doc, target_os = "linux"))]
        if self.as_os_str().as_bytes().first() == Some(&0x40) {
            let mut without_at_sign = self.into_vec();
            without_at_sign.remove(0);
            return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(
                without_at_sign,
            )?)));
        }
        Ok(UdSocketPath::File(Cow::Owned(CString::new(
            self.into_vec(),
        )?)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a Path {
    /// Converts a borrowed [`Path`] to a borrowed [`UdSocketPath::File`] with the same lifetime.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
    /// [`UdSocketPath::File`]: struct.UdSocketPath.html#file " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        if self.as_os_str().as_bytes().last() != Some(&0) {
            let osstring = self.to_owned().into_os_string().into_vec();
            let cstring = CString::new(osstring)?;
            Ok(UdSocketPath::File(Cow::Owned(cstring)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_os_str().as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for PathBuf {
    /// Converts an owned [`PathBuf`] to an owned [`UdSocketPath::File`].
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
    /// [`UdSocketPath::File`]: struct.UdSocketPath.html#file " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        let cstring = CString::new(self.into_os_string().into_vec())?;
        Ok(UdSocketPath::File(Cow::Owned(cstring)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a str {
    /// Converts a borrowed [`str`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end. This is done to support normal string literals, since adding `\0` at the end of every single socket name string is tedious and unappealing.
    ///
    /// [`str`]: https://doc.rust-lang.org/std/primitive.str.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        // Use chars().next() instead of raw indexing to account for UTF-8 with BOM
        #[cfg(any(doc, target_os = "linux"))]
        if self.starts_with('@') {
            if !self.ends_with('\0') {
                let mut owned = self.to_owned();
                owned.remove(0);
                return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(owned)?)));
            } else {
                let without_at_sign = self.split_at(1).0;
                let cstr = CStr::from_bytes_with_nul(without_at_sign.as_bytes())
                    .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
                return Ok(UdSocketPath::Namespaced(Cow::Borrowed(cstr)));
            }
        }
        if !self.ends_with('\0') {
            Ok(UdSocketPath::File(Cow::Owned(CString::new(
                self.to_owned(),
            )?)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for String {
    /// Converts an owned [`String`] to an owned `UdSocketPath`. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    #[inline]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(any(doc, target_os = "linux"))]
        if self.starts_with('@') {
            let mut without_at_sign = self;
            without_at_sign.remove(0);
            return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(
                without_at_sign.into_bytes(),
            )?)));
        }
        Ok(UdSocketPath::File(Cow::Owned(CString::new(
            self.into_bytes(),
        )?)))
    }
}

/// Ancillary data to be sent through a Unix domain socket or read from an input buffer.
///
/// Ancillary data gives unique possibilities to Unix domain sockets which no other POSIX API has: passing file descriptors between two processes which do not have a parent-child relationship. It also can be used to transfer credentials of a process reliably.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AncillaryData<'a> {
    /// One or more file descriptors to be sent.
    FileDescriptors(Cow<'a, [c_int]>),
    /// Credentials to be sent. The specified values are checked by the system when sent for all users except for the superuser — for senders, this means that the correct values need to be filled out, otherwise, an error is returned; for receivers, this means that the credentials are to be trusted for authentification purposes. For convenience, the [`credentials`] function provides a value which is known to be valid when sent.
    ///
    /// [`credentials`]: #method.credentials " "
    #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
    #[cfg_attr(
        feature = "doc_cfg",
        doc(cfg(not(any(target_os = "macos", target_os = "ios",)),))
    )]
    Credentials {
        /// The process identificator (PID) for the process.
        pid: pid_t,
        /// The user identificator (UID) of the user who started the process.
        uid: uid_t,
        /// The group identificator (GID) of the user who started the process.
        gid: gid_t,
    },
}
impl<'a> AncillaryData<'a> {
    /// The size of a single `AncillaryData::Credentials` element when packed into the Unix ancillary data format. Useful for allocating a buffer when you expect to receive credentials.
    pub const ENCODED_SIZE_OF_CREDENTIALS: usize = Self::_ENCODED_SIZE_OF_CREDENTIALS;
    cfg_if! {
        if #[cfg(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
            ))
        ))] {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = mem::size_of::<cmsghdr>() + mem::size_of::<ucred>();
        } else if #[cfg(all(
            unix,
            any(
                target_os = "macos",
                target_os = "ios",
            ))
        )] {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = mem::size_of::<cmsghdr>();
        } else {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = 0;
        }
    }

    /// Calculates the size of an `AncillaryData::FileDescriptors` element with the specified amount of file descriptors when packed into the Unix ancillary data format. Useful for allocating a buffer when you expect to receive a specific amount of file descriptors.
    #[inline(always)]
    pub const fn encoded_size_of_file_descriptors(num_descriptors: usize) -> usize {
        #[cfg(not(doc))]
        {
            mem::size_of::<cmsghdr>() + num_descriptors * 4
        }
        #[cfg(doc)]
        0
    }

    /// Inexpensievly clones `self` by borrowing the `FileDescriptors` variant or copying the `Credentials` variant.
    #[inline]
    pub fn clone_ref(&'a self) -> Self {
        match *self {
            Self::FileDescriptors(ref fds) => Self::FileDescriptors(Cow::Borrowed(&fds)),
            #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
            Self::Credentials { pid, uid, gid } => Self::Credentials { pid, uid, gid },
        }
    }

    /// Returns the size of an ancillary data element when packed into the Unix ancillary data format.
    #[inline]
    pub fn encoded_size(&self) -> usize {
        match self {
            Self::FileDescriptors(fds) => Self::encoded_size_of_file_descriptors(fds.len()),
            #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
            Self::Credentials { .. } => Self::ENCODED_SIZE_OF_CREDENTIALS,
        }
    }

    /// Encodes the ancillary data into `EncodedAncillaryData` which is ready to be sent via a Unix domain socket.
    fn encode(op: impl IntoIterator<Item = Self>) -> EncodedAncillaryData<'static> {
        let items = op.into_iter();
        let mut buffer = Vec::with_capacity(
            {
                let size_hint = items.size_hint();
                size_hint.1.unwrap_or(size_hint.0)
                // If we assume that all ancillary data elements are credentials, we're more than fine.
            } * Self::ENCODED_SIZE_OF_CREDENTIALS,
        );
        for i in items {
            let mut cmsg_len = mem::size_of::<cmsghdr>();
            let cmsg_level_bytes = SOL_SOCKET.to_ne_bytes();
            let cmsg_type_bytes;

            match i {
                AncillaryData::FileDescriptors(fds) => {
                    cmsg_type_bytes = SCM_RIGHTS.to_ne_bytes();
                    cmsg_len += fds.len() * 4;
                    // #[cfg(target_pointer_width = "64")]
                    // this was here, I don't even remember why, but that
                    // wouldn't compile on a 32-bit machine
                    let cmsg_len_bytes = cmsg_len.to_ne_bytes();
                    buffer.extend_from_slice(&cmsg_len_bytes);
                    buffer.extend_from_slice(&cmsg_level_bytes);
                    buffer.extend_from_slice(&cmsg_type_bytes);
                    for i in fds.iter().copied() {
                        let desc_bytes = i.to_ne_bytes();
                        buffer.extend_from_slice(&desc_bytes);
                    }
                }
                #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
                AncillaryData::Credentials { pid, uid, gid } => {
                    cmsg_type_bytes = SCM_RIGHTS.to_ne_bytes();
                    cmsg_len += mem::size_of::<ucred>();
                    // #[cfg(target_pointer_width = "64")]
                    let cmsg_len_bytes = cmsg_len.to_ne_bytes();
                    let pid_bytes = pid.to_ne_bytes();
                    let uid_bytes = uid.to_ne_bytes();
                    let gid_bytes = gid.to_ne_bytes();
                    buffer.extend_from_slice(&cmsg_len_bytes);
                    buffer.extend_from_slice(&cmsg_level_bytes);
                    buffer.extend_from_slice(&cmsg_type_bytes);
                    buffer.extend_from_slice(&pid_bytes);
                    buffer.extend_from_slice(&uid_bytes);
                    buffer.extend_from_slice(&gid_bytes);
                }
            }
        }
        EncodedAncillaryData(Cow::Owned(buffer))
    }
}
impl AncillaryData<'static> {
    /// Fetches the credentials of the process from the system and returns a value which can be safely sent to another process without the system complaining about an unauthorized attempt to impersonate another process/user/group.
    ///
    /// If you want to send credentials to another process, this is usually the function you need to obtain the desired ancillary payload.
    #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
    #[cfg_attr(
        feature = "doc_cfg",
        doc(cfg(not(any(target_os = "macos", target_os = "ios",)),))
    )]
    #[inline]
    pub fn credentials() -> Self {
        Self::Credentials {
            pid: unsafe { libc::getpid() },
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
        }
    }
}

/// A buffer used for sending ancillary data into Unix domain sockets.
#[repr(transparent)]
#[derive(Clone, Debug)]
struct EncodedAncillaryData<'a>(pub Cow<'a, [u8]>);
impl<'a> From<&'a [u8]> for EncodedAncillaryData<'a> {
    #[inline(always)]
    fn from(op: &'a [u8]) -> Self {
        Self(Cow::Borrowed(op))
    }
}
impl From<Vec<u8>> for EncodedAncillaryData<'static> {
    #[inline(always)]
    fn from(op: Vec<u8>) -> Self {
        Self(Cow::Owned(op))
    }
}
impl<'b> FromIterator<AncillaryData<'b>> for EncodedAncillaryData<'static> {
    #[inline(always)]
    fn from_iter<I: IntoIterator<Item = AncillaryData<'b>>>(iter: I) -> Self {
        AncillaryData::encode(iter)
    }
}
impl<'b> From<Vec<AncillaryData<'b>>> for EncodedAncillaryData<'static> {
    #[inline(always)]
    fn from(op: Vec<AncillaryData<'b>>) -> Self {
        Self::from_iter(op)
    }
}
impl<'b: 'c, 'c> From<&'c [AncillaryData<'b>]> for EncodedAncillaryData<'static> {
    #[inline(always)]
    fn from(op: &'c [AncillaryData<'b>]) -> Self {
        op.iter().map(AncillaryData::clone_ref).collect::<Self>()
    }
}
impl<'a> AsRef<[u8]> for EncodedAncillaryData<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// A buffer used for receiving ancillary data from Unix domain sockets.
///
/// The actual ancillary data can be obtained using the [`decode`] method.
///
/// # Example
/// See [`UdStream`] or [`UdStreamListener`] for an example of receiving ancillary data.
///
/// [`decode`]: #method.decode " "
/// [`UdStream`]: struct.UdStream.html#examples " "
/// [`UdStreamListener`]: struct.UdStreamListener.html#examples " "
#[derive(Debug)]
pub enum AncillaryDataBuf<'a> {
    /// The buffer's storage is borrowed.
    Borrowed(&'a mut [u8]),
    /// The buffer's storage is owned by the buffer itself.
    Owned(Vec<u8>),
}
impl<'a> AncillaryDataBuf<'a> {
    /// Creates an owned ancillary data buffer with the specified capacity.
    #[inline(always)]
    pub fn owned_with_capacity(capacity: usize) -> Self {
        Self::Owned(Vec::with_capacity(capacity))
    }
    /// Creates a decoder which decodes the ancillary data buffer into a friendly representation of its contents.
    ///
    /// All invalid ancillary data blocks are skipped — if there was garbage data in the buffer to begin with, the resulting buffer will either be empty or contain invalid credentials/file descriptors. This should normally never happen if the data is actually received from a Unix domain socket.
    #[inline(always)]
    pub fn decode(&'a self) -> AncillaryDataDecoder<'a> {
        AncillaryDataDecoder {
            buffer: self.as_ref(),
            i: 0,
        }
    }
}
impl<'a> From<&'a mut [u8]> for AncillaryDataBuf<'a> {
    #[inline(always)]
    fn from(op: &'a mut [u8]) -> Self {
        Self::Borrowed(op)
    }
}
impl From<Vec<u8>> for AncillaryDataBuf<'static> {
    #[inline(always)]
    fn from(op: Vec<u8>) -> Self {
        Self::Owned(op)
    }
}
impl<'a> From<&'a mut AncillaryDataBuf<'a>> for AncillaryDataBuf<'a> {
    #[inline]
    fn from(op: &'a mut AncillaryDataBuf<'a>) -> Self {
        match op {
            Self::Borrowed(slice) => Self::Borrowed(slice),
            Self::Owned(vec) => Self::Borrowed(vec),
        }
    }
}
impl<'a> AsRef<[u8]> for AncillaryDataBuf<'a> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Borrowed(slice) => slice,
            Self::Owned(vec) => vec,
        }
    }
}
impl<'a> AsMut<[u8]> for AncillaryDataBuf<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Borrowed(slice) => slice,
            Self::Owned(vec) => vec,
        }
    }
}

/// An iterator which decodes ancillary data from an ancillary data buffer.
///
/// This iterator is created by the [`decode`] method on [`AncillaryDataBuf`] — see its documentation for more.
///
/// [`AncillaryDataBuf`]: struct.AncillaryDataBuf.html " "
/// [`decode`]: struct.AncillaryDataBuf.html#method.decode " "
#[derive(Clone, Debug)]
pub struct AncillaryDataDecoder<'a> {
    buffer: &'a [u8],
    i: usize,
}
impl<'a> From<&'a AncillaryDataBuf<'a>> for AncillaryDataDecoder<'a> {
    #[inline(always)]
    fn from(buffer: &'a AncillaryDataBuf<'a>) -> Self {
        buffer.decode()
    }
}
impl<'a> Iterator for AncillaryDataDecoder<'a> {
    type Item = AncillaryData<'static>;
    fn next(&mut self) -> Option<Self::Item> {
        #[inline(always)]
        fn u32_from_slice(bytes: &[u8]) -> u32 {
            u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
        #[inline(always)]
        fn u64_from_slice(bytes: &[u8]) -> u64 {
            u64::from_ne_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        }
        let bytes = self.buffer;
        let end = bytes.len() - 1;

        if let Some(diff) = bytes.len().checked_sub(self.i) {
            if diff == 0 {
                self.i = end;
                return None;
            }
        } else {
            self.i = end;
            return None;
        }

        // The first field is the length, which is a size_t
        #[cfg(target_pointer_width = "64")]
        let element_size = {
            if bytes.len() - self.i < 8 {
                self.i = end;
                return None;
            }
            u64_from_slice(&bytes[self.i..self.i + 8]) as usize
        };
        #[cfg(target_pointer_width = "32")]
        let element_size = {
            if bytes.len() - self.i < 4 {
                self.i = end;
                return None;
            }
            u32_from_slice(&bytes[self.i..self.i + 4]) as usize
        };
        // The cmsg_level field is always SOL_SOCKET — we don't need it, let's get the
        // cmsg_type field right away by first getting the offset at which it's
        // located:
        #[cfg(target_pointer_width = "64")]
        let type_offset: usize = 8 + 4; // 8 for cmsg_size, 4 for cmsg_level
        #[cfg(target_pointer_width = "32")]
        let type_offset: usize = 4 + 4; // 4 for cmsg_size, 4 for cmsg_level

        // Now let's get the type itself:
        let element_type = u32_from_slice(&bytes[self.i + type_offset..=self.i + type_offset + 4]);
        // The size of cmsg_size, cmsg_level and cmsg_type together
        let element_offset = type_offset + 4;

        // Update the counter before returning.
        self.i += element_offset // cmsg_size, cmsg_level and cmsg_type
                + element_size; // data size

        // SAFETY: those are ints lmao
        match element_type as i32 {
            SCM_RIGHTS => {
                // We're reading one or multiple descriptors from the ancillary data payload.
                // All descriptors are 4 bytes in size — leftover bytes are discarded thanks
                // to integer division rules
                let amount_of_descriptors = element_size / 4;
                let mut descriptors = Vec::<c_int>::with_capacity(amount_of_descriptors);
                let mut descriptor_offset = element_offset;
                for _ in 0..amount_of_descriptors {
                    descriptors.push(
                        // SAFETY: see above
                        u32_from_slice(&bytes[descriptor_offset..descriptor_offset + 4]) as i32,
                    );
                    descriptor_offset += 4;
                }
                Some(AncillaryData::FileDescriptors(Cow::Owned(descriptors)))
            }
            #[cfg(any(doc, not(any(target_os = "macos", target_os = "ios",)),))]
            SCM_CREDENTIALS => {
                // We're reading a single ucred structure from the ancillary data payload.
                // SAFETY: those are still ints
                let pid_offset = element_offset;
                let pid: pid_t = unsafe {
                    mem::transmute::<u32, i32>(u32_from_slice(&bytes[pid_offset..pid_offset + 4]))
                };
                let uid_offset = pid_offset + 4;
                let uid: uid_t = u32_from_slice(&bytes[uid_offset..uid_offset + 4]);
                let gid_offset = uid_offset + 4;
                let gid: gid_t = u32_from_slice(&bytes[gid_offset..gid_offset + 4]);
                Some(AncillaryData::Credentials { pid, uid, gid })
            }
            _ => self.next(), // Do nothing if we hit corrupted data.
        }
    }
}
impl FusedIterator for AncillaryDataDecoder<'_> {}

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
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
impl FusedIterator for Incoming<'_> {}
impl<'a> From<&'a UdStreamListener> for Incoming<'a> {
    #[inline(always)]
    fn from(listener: &'a UdStreamListener) -> Self {
        Self { listener }
    }
}
