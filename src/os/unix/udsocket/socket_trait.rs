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
    /// Fetches the credentials of the other end of the connection without using ancillary data. The set of credentials
    /// returned depends on the platform.
    ///
    /// # Implementation
    /// The credential tables used are as follows:
    /// - **Linux:** `ucred` (PID, UID, GID)
    /// - **FreeBSD, DragonFly BSD, Apple:** `xucred` (effective UID, up to 16 supplementary groups)
    #[cfg_attr(
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        )))
    )]
    #[cfg(any(uds_ucred, uds_xucred))]
    #[inline]
    fn get_peer_credentials(&self) -> io::Result<credentials::Credentials<'static>> {
        use credentials::{Credentials, CredentialsInner};
        #[cfg(uds_ucred)]
        let cred = {
            let ucred = c_wrappers::get_peer_ucred(self.as_fd())?;
            CredentialsInner::Ucred(ucred)
        };
        #[cfg(uds_xucred)]
        let cred = {
            let xucred = c_wrappers::get_peer_xucred(self.as_fd())?;
            CredentialsInner::Xucred(xucred)
        };
        Ok(Credentials(cred))
    }
    /// Enables or disables continuous reception of credentials via ancillary data.
    ///
    /// After this option is set to `true`, every ancillary-enabled receive call will return a table of credentials of
    /// the process on the other side, directly associated with the data being received.
    ///
    /// Note that this has absolutely no effect on explicit sending of credentials – that can be done regardless of
    /// whether this option is enabled.
    #[cfg_attr( // uds_cont_credentials template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
            target_os = "freebsd",
        )))
    )]
    #[cfg(uds_cont_credentials)]
    #[inline]
    fn set_continuous_ancillary_credentials(&self, val: bool) -> io::Result<()> {
        c_wrappers::set_continuous_ancillary_cred(self.as_fd(), val)
    }
    /// Enables or disables one-time reception of credentials via ancillary data.
    ///
    /// After this option is set to `true`, the next ancillary-enabled receive call will return a table of credentials
    /// of the process on the other side, directly associated with the data being received. The operation, upon
    /// successful return from the kernel, will already have atomically set this option back to `false`.
    ///
    /// Note that this has absolutely no effect on explicit sending of credentials – that can be done regardless of
    /// whether this option is enabled.
    #[cfg_attr( // uds_sockcred template
        feature = "doc_cfg",
        doc(cfg(target_os = "netbsd"))
    )]
    #[cfg(uds_sockcred)]
    #[inline]
    fn set_oneshot_ancillary_credentials(&self, val: bool) -> io::Result<()> {
        c_wrappers::set_oneshot_ancillary_cred(self.as_fd(), val)
    }
}

impl UdSocket for UdStream {}
impl UdSocket for UdDatagram {}
#[cfg(feature = "tokio")]
impl UdSocket for super::tokio::UdStream {}
#[cfg(feature = "tokio")]
impl UdSocket for super::tokio::UdDatagram {}
