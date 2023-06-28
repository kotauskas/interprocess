use super::*;
use crate::os::unix::unixprelude::*;
use std::{io, net::Shutdown};

/// Common methods for non-listener Ud-sockets.
pub trait UdSocket: AsFd {
    /// Shuts down the read, write, or both halves of the stream. See [`Shutdown`].
    ///
    /// Attempting to call this method with the same `how` argument multiple times may return `Ok(())` every time or it
    /// may return an error the second time it is called, depending on the platform. You must either avoid using the
    /// same value twice or ignore the error entirely.
    #[inline]
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        c_wrappers::shutdown(self.as_fd(), how)
    }
    /// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
    ///
    /// In nonblocking mode, calls to the `recv…` methods and the [`Read`](io::Read) trait methods will never wait for
    /// at least one byte of data to become available; calls to `send…` methods and the [`Write`](io::Write) trait
    /// methods will never wait for the other side to remove enough bytes from the buffer for the write operation to be
    /// performed. Those operations will instead return a [`WouldBlock`](io::ErrorKind::WouldBlock) error immediately,
    /// allowing the thread to perform other useful operations in the meantime.
    #[inline]
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        c_wrappers::set_nonblocking(self.as_fd(), nonblocking)
    }
    /// Checks whether the stream is currently in nonblocking mode or not.
    #[inline]
    fn is_nonblocking(&self) -> io::Result<bool> {
        c_wrappers::get_nonblocking(self.as_fd())
    }
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure
    /// contains the process identifier, user identifier and group identifier of the peer.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    fn get_peer_credentials(&self) -> io::Result<libc::ucred> {
        c_wrappers::get_peer_ucred(self.as_fd())
    }
}

impl UdSocket for UdStream {}
impl UdSocket for UdDatagram {}
