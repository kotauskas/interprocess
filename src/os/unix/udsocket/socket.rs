#[cfg(uds_peercred)]
use super::util::get_peer_ucred;
use super::{
    super::{close_by_error, handle_fd_error},
    imports::*,
    util::{
        check_ancillary_unsound, enable_passcred, fill_out_msghdr_r, mk_msghdr_r, mk_msghdr_w,
        raw_get_nonblocking, raw_set_nonblocking,
    },
    AncillaryData, AncillaryDataBuf, EncodedAncillaryData, ToUdSocketPath, UdSocketPath,
};
use crate::Sealed;
#[cfg(any(doc, target_os = "linux"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
use crate::ReliableReadMsg;
use std::{
    fmt::{self, Debug, Formatter},
    io::{self, IoSlice, IoSliceMut},
    iter,
    mem::{size_of, size_of_val, zeroed},
};
use to_method::To;

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
        let addr = path.to_socket_path()?.try_to::<sockaddr_un>()?;
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
            libc::bind(
                socket,
                // Double cast because you cannot cast a reference to a pointer of arbitrary type
                // but you can cast any narrow pointer to any other narrow pointer
                &addr as *const _ as *const _,
                size_of::<sockaddr_un>() as u32,
            )
        } != -1;
        if !success {
            unsafe { return Err(handle_fd_error(socket)) };
        }
        unsafe { enable_passcred(socket).map_err(close_by_error(socket))? };
        Ok(unsafe {
            // SAFETY: we just created the file descriptor, meaning that it's guaranteeed
            // not to be used elsewhere
            Self::from_raw_fd(socket)
        })
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
    pub fn connect<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        let path = path.to_socket_path()?; // Shadow original by conversion
        let (addr, addrlen) = unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            (addr, size_of::<sockaddr_un>())
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
        let success =
            unsafe { libc::connect(socket, &addr as *const _ as *const _, addrlen as u32) } != -1;
        if !success {
            unsafe { return Err(handle_fd_error(socket)) };
        }
        unsafe { enable_passcred(socket).map_err(close_by_error(socket))? };
        Ok(unsafe { Self::from_raw_fd(socket) })
    }

    fn add_fake_trunc_flag(x: usize) -> (usize, bool) {
        (x, false)
    }

    /// Receives a single datagram from the socket, returning the size of the received datagram.
    ///
    /// *Note: there is an additional meaningless boolean return value which is always `false`. It used to signify whether the datagram was truncated or not, but the functionality was implemented incorrectly and only on Linux, leading to its removal in version 1.2.0. In the next breaking release, 2.0.0, the return value will be changed to just `io::Result<usize>`.*
    ///
    /// # System calls
    /// - `read`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, bool)> {
        self.fd.read(buf).map(Self::add_fake_trunc_flag)
    }

    /// Receives a single datagram from the socket, making use of [scatter input] and returning the size of the received datagram.
    ///
    /// *Note: there is an additional meaningless boolean return value which is always `false`. It used to signify whether the datagram was truncated or not, but the functionality was implemented incorrectly and only on Linux, leading to its removal in version 1.2.0. In the next breaking release, 2.0.0, the return value will be changed to just `io::Result<usize>`.*
    ///
    /// # System calls
    /// - `readv`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<(usize, bool)> {
        self.fd.read_vectored(bufs).map(Self::add_fake_trunc_flag)
    }

    /// Receives a single datagram and ancillary data from the socket. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - *Deprecated `bool` field (always `false`), see note*
    /// - How many bytes of ancillary data were received
    /// - *Another deprecated `bool` field (always `false`), see note*
    ///
    /// *Note: there are two additional meaningless boolean return values which are always `false`. They used to signify whether the datagram, and the ancillary data respectively, were truncated or not, but the functionality was implemented incorrectly and only on Linux, leading to its removal in version 1.2.0. In the next breaking release, 2.0.0, the return value will be changed to just `io::Result<usize>`.*
    ///
    /// # System calls
    /// - `recvmsg`
    ///
    /// [scatter input]: https://en.wikipedia.org/wiki/Vectored_I/O " "
    pub fn recv_ancillary<'a: 'b, 'b>(
        &self,
        buf: &mut [u8],
        abuf: &'b mut AncillaryDataBuf<'a>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        check_ancillary_unsound()?;
        self.recv_ancillary_vectored(&mut [IoSliceMut::new(buf)], abuf)
    }

    /// Receives a single datagram and ancillary data from the socket, making use of [scatter input]. The return value is in the following order:
    /// - How many bytes of the datagram were received
    /// - *Deprecated `bool` field (always `false`), see note*
    /// - How many bytes of ancillary data were received
    /// - *Another deprecated `bool` field (always `false`), see note*
    ///
    /// *Note: there are two additional meaningless boolean return values which are always `false`. They used to signify whether the datagram, and the ancillary data respectively, were truncated or not, but the functionality was implemented incorrectly and only on Linux, leading to its removal in version 1.2.0. In the next breaking release, 2.0.0, the return value will be changed to just `io::Result<(usize, usize)>`.*
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
        check_ancillary_unsound()?;
        let mut hdr = mk_msghdr_r(bufs, abuf.as_mut())?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, result as usize)
        };
        if success {
            Ok((bytes_read, false, hdr.msg_controllen as _, false))
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
    /// - *Deprecated `bool` field (always `false`), see note*
    /// - How many bytes of ancillary data were received
    /// - *Another deprecated `bool` field (always `false`), see note*
    ///
    /// *Note: there are two additional meaningless boolean return values which are always `false`. They used to signify whether the datagram, and the ancillary data respectively, were truncated or not, but the functionality was implemented incorrectly and only on Linux, leading to its removal in version 1.2.0. In the next breaking release, 2.0.0, the return value will be changed to just `io::Result<(usize, usize)>`.*
    ///
    /// # System calls
    /// - `recvmsg`
    pub fn recv_from_ancillary<'a: 'b, 'b, 'c: 'd, 'd>(
        &self,
        buf: &mut [u8],
        abuf: &'b mut AncillaryDataBuf<'a>,
        addr_buf: &'d mut UdSocketPath<'c>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        if !abuf.as_ref().is_empty() {
            // Branching required because recv_from_vectored always uses
            // recvmsg (no non-ancillary counterpart)
            check_ancillary_unsound()?;
        }
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
    pub fn recv_from_ancillary_vectored<'a: 'b, 'b, 'c: 'd, 'd>(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        abuf: &'b mut AncillaryDataBuf<'a>,
        addr_buf: &'d mut UdSocketPath<'c>,
    ) -> io::Result<(usize, bool, usize, bool)> {
        check_ancillary_unsound()?;
        // SAFETY: msghdr consists of integers and pointers, all of which are nullable
        let mut hdr = unsafe { zeroed::<msghdr>() };
        // Same goes for sockaddr_un
        let mut addr_buf_staging = unsafe { zeroed::<sockaddr_un>() };
        // It's a void* so the doublecast is mandatory
        hdr.msg_name = &mut addr_buf_staging as *mut _ as *mut _;
        hdr.msg_namelen = size_of_val(&addr_buf_staging).try_to::<u32>().unwrap();
        fill_out_msghdr_r(&mut hdr, bufs, abuf.as_mut())?;
        let (success, bytes_read) = unsafe {
            let result = libc::recvmsg(self.as_raw_fd(), &mut hdr as *mut _, 0);
            (result != -1, result as usize)
        };
        let path_length = hdr.msg_namelen as usize;
        if success {
            addr_buf.write_sockaddr_un_to_self(&addr_buf_staging, path_length);
            Ok((bytes_read, false, hdr.msg_controllen as _, false))
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
                libc::MSG_TRUNC | libc::MSG_PEEK,
            );
            (size != -1, size as usize)
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
    pub fn send_ancillary<'a>(
        &self,
        buf: &[u8],
        ancillary_data: impl IntoIterator<Item = AncillaryData<'a>>,
    ) -> io::Result<(usize, usize)> {
        check_ancillary_unsound()?;
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

    /// Enables or disables the nonblocking mode for the socket. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to the `recv…` methods and the `Read` trait methods will never wait for at least one message to become available; calls to `send…` methods and the `Write` trait methods will never wait for the other side to remove enough bytes from the buffer for the write operation to be performed. Those operations will instead return a [`WouldBlock`] error immediately, allowing the thread to perform other useful operations in the meantime.
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
impl Debug for UdSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdSocket")
            .field("file_descriptor", &self.as_raw_fd())
            .finish()
    }
}
#[cfg(any(doc, target_os = "linux"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(target_os = "linux")))]
impl ReliableReadMsg for UdSocket {
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
        let msg_size = self.peek_msg_size()?;
        if msg_size > buf.len() {
            let mut new_buffer = vec![0; msg_size];
            let len = self.recv(&mut new_buffer)?.0;
            new_buffer.truncate(len);
            Ok(Err(new_buffer))
        } else {
            Ok(Ok(self.recv(buf)?.0))
        }
    }
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
        let msg_size = self.peek_msg_size()?;
        if msg_size > buf.len() {
            Ok(Err(msg_size))
        } else {
            Ok(Ok(self.recv(buf)?.0))
        }
    }
}
#[cfg(unix)]
impl Sealed for UdSocket {}
#[cfg(unix)]
impl AsRawFd for UdSocket {
    fn as_raw_fd(&self) -> c_int {
        self.fd.as_raw_fd()
    }
}
#[cfg(unix)]
impl IntoRawFd for UdSocket {
    fn into_raw_fd(self) -> c_int {
        self.fd.into_raw_fd()
    }
}
#[cfg(unix)]
impl FromRawFd for UdSocket {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self { fd: FdOps::new(fd) }
    }
}
