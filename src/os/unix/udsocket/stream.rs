#[cfg(uds_peercred)]
use super::util::get_peer_ucred;
use super::{
    super::{close_by_error, handle_fd_error},
    imports::*,
    util::{enable_passcred, mk_msghdr_r, mk_msghdr_w, raw_get_nonblocking, raw_set_nonblocking},
    AncillaryData, AncillaryDataBuf, EncodedAncillaryData, ToUdSocketPath,
};
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, IoSlice, IoSliceMut, Read, Write},
    iter,
    mem::size_of,
};
use to_method::To;

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
/// conn.write_all(b"Hello from client!")?;
/// let mut string_buffer = String::new();
/// conn.read_to_string(&mut string_buffer)?;
/// println!("Server answered: {}", string_buffer);
/// # }
/// # Ok(()) }
/// ```
///
/// Receiving and sending ancillary data:
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(uds_scm_credentials)] {
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
///     file.write(b"Hello foreign file descriptor!\n")?;
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
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let addr = path.to_socket_path()?.try_to::<sockaddr_un>()?;
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
            libc::connect(
                socket,
                &addr as *const _ as *const _,
                size_of::<sockaddr_un>() as u32,
            )
        } != 1;
        if !success {
            unsafe { return Err(handle_fd_error(socket)) };
        }
        unsafe { enable_passcred(socket).map_err(close_by_error(socket))? };
        Ok(unsafe { Self::from_raw_fd(socket) })
    }

    /// Receives bytes from the socket stream.
    ///
    /// # System calls
    /// - `read`
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
    #[allow(clippy::useless_conversion)]
    pub fn recv_ancillary_vectored<'a: 'b, 'b>(
        &self,
        bufs: &[IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, usize)> {
        let mut hdr = mk_msghdr_r(bufs, abuf.as_mut())?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, result as usize)
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
    /// - `write`
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
    #[allow(clippy::useless_conversion)]
    pub fn send_ancillary_vectored<'a>(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        let abuf = ancillary_data
            .into_iter()
            .collect::<EncodedAncillaryData<'_>>();
        let hdr = mk_msghdr_w(bufs, abuf.as_ref())?;
        let (success, bytes_written) = unsafe {
            let result = libc::sendmsg(self.as_raw_fd(), &hdr as *const _, 0);
            (result != -1, result as usize)
        };
        if success {
            Ok((bytes_written, hdr.msg_controllen as _))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to the `recv…` methods and the `Read` trait methods will never wait for at least one byte of data to become available; calls to `send…` methods and the `Write` trait methods will never wait for the other side to remove enough bytes from the buffer for the write operation to be performed. Those operations will instead return a [`WouldBlock`] error immediately, allowing the thread to perform other useful operations in the meantime.
    ///
    /// [`accept`]: #method.accept " "
    /// [`incoming`]: #method.incoming " "
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe { raw_set_nonblocking(self.fd.0, nonblocking) }
    }
    /// Checks whether the stream is currently in nonblocking mode or not.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        unsafe { raw_get_nonblocking(self.fd.0) }
    }

    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(any(doc, uds_peercred))]
    #[cfg_attr( // uds_peercred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox",
            target_os = "haiku"
        )))
    )]
    pub fn get_peer_credentials(&self) -> io::Result<ucred> {
        unsafe { get_peer_ucred(self.fd.0) }
    }
}
impl Read for UdStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.fd.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut abuf = AncillaryDataBuf::Owned(Vec::new());
        self.recv_ancillary_vectored(bufs, &mut abuf).map(|x| x.0)
    }
}
impl Write for UdStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.fd.write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_ancillary_vectored(bufs, iter::empty())
            .map(|x| x.0)
    }
    fn flush(&mut self) -> io::Result<()> {
        // You cannot flush a socket
        Ok(())
    }
}
impl Debug for UdStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdStream")
            .field("file_descriptor", &self.as_raw_fd())
            .finish()
    }
}
#[cfg(unix)]
impl AsRawFd for UdStream {
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
#[cfg(unix)]
impl IntoRawFd for UdStream {
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for UdStream {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps::new(fd) }
    }
}
