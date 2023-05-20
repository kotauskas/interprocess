use super::{
    c_wrappers,
    cmsg::{CmsgMut, CmsgRef},
    util::{make_msghdr_r, make_msghdr_w},
    PathDropGuard, ToUdSocketPath, UdSocketPath,
};
use crate::os::unix::{unixprelude::*, FdOps};
#[cfg(target_os = "linux")]
use crate::{
    reliable_recv_msg::{ReliableRecvMsg, TryRecvResult},
    Sealed,
};
use libc::{sockaddr_un, SOCK_DGRAM};
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, IoSlice, IoSliceMut},
    mem::{size_of_val, zeroed},
    os::raw::c_void,
};
use to_method::To;

/// A datagram socket in the Unix domain.
///
/// All such sockets have the `SOCK_DGRAM` socket type; in other words, this is the Unix domain version of a UDP socket.
pub struct UdSocket {
    // TODO make this not 'static
    _drop_guard: PathDropGuard<'static>,
    fd: FdOps,
}
impl UdSocket {
    /// Creates a new socket that can be referred to by the specified path.
    ///
    /// If the socket path exceeds the [maximum socket path length] (which includes the first 0 byte when using the [socket namespace]), an error is returned. Errors can also be produced for different reasons, i.e. errors should always be handled regardless of whether the path is known to be short enough or not.
    ///
    /// After the socket is dropped, the socket file will be left over. Use [`bind_with_drop_guard()`](Self::bind_with_drop_guard) to mitigate this automatically, even during panics (if unwinding is enabled).
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
        Self::_bind(path.to_socket_path()?, false)
    }
    /// Creates a new socket that can be referred to by the specified path, remembers the address, and installs a drop guard that will delete the socket file once the socket is dropped.
    ///
    /// See the documentation of [`bind()`](Self::bind).
    pub fn bind_with_drop_guard<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?, true)
    }
    fn _bind(path: UdSocketPath<'_>, keep_drop_guard: bool) -> io::Result<Self> {
        let addr = path.borrow().try_to::<sockaddr_un>()?;

        let fd = c_wrappers::create_uds(SOCK_DGRAM, false)?;
        unsafe {
            // SAFETY: addr is well-constructed
            c_wrappers::bind(&fd, &addr)?;
        }
        c_wrappers::set_passcred(&fd, true)?;

        let dg = if keep_drop_guard && matches!(path, UdSocketPath::File(..)) {
            PathDropGuard {
                path: path.upgrade(),
                enabled: true,
            }
        } else {
            PathDropGuard::dummy()
        };

        Ok(Self { fd, _drop_guard: dg })
    }
    /// Selects the Unix domain socket to send packets to. You can also just use [`.send_to()`](Self::send_to) instead, but supplying the address to the kernel once is more efficient.
    ///
    /// # Example
    /// ```no_run
    /// use interprocess::os::unix::udsocket::UdSocket;
    ///
    /// let conn = UdSocket::bind("/tmp/side_a.sock")?;
    /// conn.set_destination("/tmp/side_b.sock")?;
    /// // Communicate with datagrams here!
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    ///
    /// # System calls
    /// - `connect`
    pub fn set_destination<'a>(&self, path: impl ToUdSocketPath<'a>) -> io::Result<()> {
        let path = path.to_socket_path()?;
        self._set_destination(&path)
    }
    fn _set_destination(&self, path: &UdSocketPath<'_>) -> io::Result<()> {
        let addr = path.borrow().try_to::<sockaddr_un>()?;

        unsafe {
            // SAFETY: addr is well-constructed
            c_wrappers::connect(&self.fd, &addr)?;
        }

        Ok(())
    }

    /// Receives a single datagram from the socket, returning the size of the received datagram.
    ///
    /// # System calls
    /// - `read`
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.fd.read(buf)
    }

    /// Receives a single datagram from the socket, making use of [scatter input] and returning the size of the received datagram.
    ///
    /// # System calls
    /// - `readv`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.fd.read_vectored(bufs)
    }

    /// Receives a single datagram and ancillary data from the socket. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - How many bytes of ancillary data were received
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_ancillary(&self, buf: &mut [u8], abuf: &mut CmsgMut<'_>) -> io::Result<(usize, usize)> {
        self.recv_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf)
    }

    /// Receives a single datagram and ancillary data from the socket, making use of [scatter input]. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - How many bytes of ancillary data were received
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_ancillary_vectored(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut CmsgMut<'_>,
    ) -> io::Result<(usize, usize)> {
        let mut hdr = make_msghdr_r(bufs, abuf)?;

        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, result as usize)
        };
        ok_or_ret_errno!(success => (bytes_read, hdr.msg_controllen as _))
    }

    /// Receives a single datagram and the source address from the socket, returning how much of the buffer was filled out.
    ///
    /// # System calls
    /// - `recvmsg`
    ///     - Future versions of `interprocess` may use `recvfrom` instead; for now, this method is a wrapper around [`recv_from_vectored`].
    ///
    /// [`recv_from_vectored`]: #method.recv_from_vectored " "
    // TODO use recvfrom
    pub fn recv_from<'a: 'b, 'b>(&self, buf: &mut [u8], addr_buf: &'b mut UdSocketPath<'a>) -> io::Result<usize> {
        self.recv_from_vectored(&mut [IoSliceMut::new(buf)], addr_buf)
    }

    /// Receives a single datagram and the source address from the socket, making use of [scatter input] and returning how much of the buffer was filled out.
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_from_vectored<'a: 'b, 'b>(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        addr_buf: &'b mut UdSocketPath<'a>,
    ) -> io::Result<usize> {
        self.recv_from_ancillary_vectored(bufs, &mut CmsgMut::new(&mut []), addr_buf)
            .map(|x| x.0)
    }

    /// Receives a single datagram, ancillary data and the source address from the socket. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - How many bytes of ancillary data were received
    ///
    /// # System calls
    /// - `recvmsg`
    #[inline]
    pub fn recv_from_ancillary(
        &self,
        buf: &mut [u8],
        abuf: &mut CmsgMut<'_>,
        addr_buf: &mut UdSocketPath<'_>,
    ) -> io::Result<(usize, usize)> {
        self.recv_from_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf, addr_buf)
    }

    /// Receives a single datagram, ancillary data and the source address from the socket, making use of [scatter input]. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - How many bytes of ancillary data were received
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_from_ancillary_vectored(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &mut CmsgMut<'_>,
        addr_buf: &mut UdSocketPath<'_>,
    ) -> io::Result<(usize, usize)> {
        let mut hdr = make_msghdr_r(bufs, abuf)?;

        // SAFETY: sockaddr_un is POD
        let mut addr_buf_staging = unsafe { zeroed::<sockaddr_un>() };
        hdr.msg_name = (&mut addr_buf_staging as *mut sockaddr_un).cast::<c_void>();
        hdr.msg_namelen = size_of_val(&addr_buf_staging).try_to::<u32>().unwrap();

        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, result as usize)
        };
        let path_length = hdr.msg_namelen as usize;
        if success {
            addr_buf.write_sockaddr_un_to_self(&addr_buf_staging, path_length);
            Ok((bytes_read, hdr.msg_controllen as _))
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
    #[cfg(target_os = "linux")]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
    pub fn peek_msg_size(&self) -> io::Result<usize> {
        let mut buffer = [0_u8; 0];
        let (success, size) = unsafe {
            let size = libc::recv(
                self.as_raw_fd(),
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                libc::MSG_TRUNC | libc::MSG_PEEK,
            );
            (size != -1, size as usize)
        };
        ok_or_ret_errno!(success => size)
    }

    /// Sends a datagram into the socket.
    ///
    /// # System calls
    /// - `write`
    #[inline]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.fd.write(buf)
    }
    // TODO sendto
    /// Sends a datagram into the socket, making use of [gather output] for the main data.
    ///
    ///
    /// # System calls
    /// - `writev`
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    #[inline]
    pub fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.fd.write_vectored(bufs)
    }
    /// Sends a datagram and ancillary data into the socket.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    #[inline]
    pub fn send_ancillary(&self, buf: &[u8], abuf: CmsgRef<'_>) -> io::Result<(usize, usize)> {
        self.send_ancillary_vectored(&[IoSlice::new(buf)], abuf)
    }
    /// Sends a datagram and ancillary data into the socket, making use of [gather output] for the main data.
    ///
    /// The ancillary data buffer is automatically converted from the supplied value, if possible. For that reason, slices and `Vec`s of `AncillaryData` can be passed directly.
    ///
    /// # System calls
    /// - `sendmsg`
    ///
    /// [gather output]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn send_ancillary_vectored(&self, bufs: &[IoSlice<'_>], abuf: CmsgRef<'_>) -> io::Result<(usize, usize)> {
        let hdr = make_msghdr_w(bufs, abuf)?;

        let (success, bytes_written) = unsafe {
            let result = libc::sendmsg(self.as_raw_fd(), &hdr as *const _, 0);
            (result != -1, result as usize)
        };
        ok_or_ret_errno!(success => (bytes_written, hdr.msg_controllen as _))
    }

    /// Enables or disables the nonblocking mode for the socket. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to the `recv…` methods and the `Read` trait methods will never wait for at least one message to become available; calls to `send…` methods and the `Write` trait methods will never wait for the other side to remove enough bytes from the buffer for the write operation to be performed. Those operations will instead return a [`WouldBlock`] error immediately, allowing the thread to perform other useful operations in the meantime.
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

    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(uds_peerucred)]
    #[cfg_attr( // uds_peerucred template
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
    pub fn get_peer_credentials(&self) -> io::Result<libc::ucred> {
        c_wrappers::get_peer_ucred(&self.fd)
    }
}

impl Debug for UdSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdSocket")
            .field("fd", &self.as_raw_fd())
            .field("has_drop_guard", &self._drop_guard.enabled)
            .finish()
    }
}

#[cfg(target_os = "linux")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
impl ReliableRecvMsg for UdSocket {
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        let mut size = self.peek_msg_size()?;
        let fit = size > buf.len();
        if fit {
            size = UdSocket::recv(self, buf)?;
        }
        Ok(TryRecvResult { size, fit })
    }
}
#[cfg(target_os = "linux")]
impl Sealed for UdSocket {}

impl AsRawFd for UdSocket {
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
impl IntoRawFd for UdSocket {
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
impl FromRawFd for UdSocket {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        let fd = unsafe { FdOps::from_raw_fd(fd) };
        Self {
            fd,
            _drop_guard: PathDropGuard::dummy(),
        }
    }
}
