#[cfg(uds_supported)]
use super::c_wrappers;
use super::{
    imports::*,
    util::{check_ancillary_unsound, mk_msghdr_r, mk_msghdr_w},
    AncillaryData, AncillaryDataBuf, EncodedAncillaryData, ToUdSocketPath, UdSocketPath,
};
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, IoSlice, IoSliceMut, Read, Write},
    iter,
    net::Shutdown,
};
use to_method::To;

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`](super::UdStreamListener) or by connecting to an existing server.
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
pub struct UdStream {
    fd: FdOps,
}
impl UdStream {
    /// Connects to a Unix domain socket server at the specified path.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `socket`
    /// - `connect`
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_connect(path.to_socket_path()?, false)
    }
    pub(crate) fn connect_nonblocking<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_connect(path.to_socket_path()?, true)
    }
    fn _connect(path: UdSocketPath<'_>, nonblocking: bool) -> io::Result<Self> {
        let addr = path.try_to::<sockaddr_un>()?;

        let fd = c_wrappers::create_uds(SOCK_STREAM, nonblocking)?;
        unsafe {
            // SAFETY: addr is well-constructed
            c_wrappers::connect(&fd, &addr)?;
        }
        c_wrappers::set_passcred(&fd, true)?;

        Ok(Self { fd })
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
    /// - `readv`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.fd.read_vectored(bufs)
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
        check_ancillary_unsound()?;
        self.recv_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf)
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
        bufs: &mut [IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, usize)> {
        check_ancillary_unsound()?;
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
    /// - `senv`
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.fd.write_vectored(bufs)
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
        check_ancillary_unsound()?;
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
        check_ancillary_unsound()?;
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

    /// Shuts down the read, write, or both halves of the stream. See [`Shutdown`].
    ///
    /// Attempting to call this method with the same `how` argument multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        c_wrappers::shutdown(&self.fd, how)
    }

    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to the `recv…` methods and the `Read` trait methods will never wait for at least one byte of data to become available; calls to `send…` methods and the `Write` trait methods will never wait for the other side to remove enough bytes from the buffer for the write operation to be performed. Those operations will instead return a [`WouldBlock`] error immediately, allowing the thread to perform other useful operations in the meantime.
    ///
    /// [`accept`]: #method.accept " "
    /// [`incoming`]: #method.incoming " "
    /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        c_wrappers::set_nonblocking(&self.fd, nonblocking)
    }
    /// Checks whether the stream is currently in nonblocking mode or not.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        c_wrappers::get_nonblocking(&self.fd)
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
        c_wrappers::get_peer_ucred(&self.fd)
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
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}

impl AsRawFd for UdStream {
    #[cfg(unix)]
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
impl IntoRawFd for UdStream {
    #[cfg(unix)]
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
impl FromRawFd for UdStream {
    #[cfg(unix)]
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps::new(fd) }
    }
}
