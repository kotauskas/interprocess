#[cfg(uds_cmsgcred)]
pub(super) mod freebsdlike;
#[cfg(uds_ucred)]
pub(super) mod ucred;
cfg_if::cfg_if! {
    if #[cfg(uds_ucred)] {
        pub(super) use ucred::{Credentials as CredentialsImpl, Groups as GroupsImpl};
    } else if #[cfg(uds_cmsgcred)] {
        pub(super) use freebsdlike::{Credentials as CredentialsImpl, Groups as GroupsImpl};
    }
}

use libc::{gid_t, pid_t, uid_t};
use std::iter::FusedIterator;

/// A table of credentials for portable secure authentication.
///
/// # Dedicated peer credentials querying
/// TODO talk here about peercred in `UdSocket`
///
/// # Ancillary message
///
/// This struct actually doubles as an ancillary data message that allows receiving the credentials of the peer process
/// and, on some systems, setting the contents of this ancillary message that the other process will receive.
///
/// To receive this message, an appropriate socket option must be enabled. There are actually two main types of such
/// socket options: one-shot credentials reception and continuous ("persistent" in FreeBSD parlance) credentials spam.
/// The latter option is primarily useful with datagram sockets, which are connectionless.
/// After one of those those types of options is enabled, either every receive operation that provides an ancillary data
/// buffer, or just the next one, will receive an instance of this message.
// TODO finish writing this
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Credentials<'a>(pub(super) CredentialsImpl<'a>);
/// Methods that read the received/stored credentials.
impl<'a> Credentials<'a> {
    /// Returns the **effective** user ID stored in the credentials table, or `None` if no such information is
    /// available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return the UID from `ucred` despite the Linux kernel allowing either the effective or the real UID
    /// to be sent.
    #[inline]
    pub fn euid(&self) -> Option<uid_t> {
        self.0.euid()
    }
    /// Returns the **real** user ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return `None`, even though `ucred` may contain either the effective or the real UID; this is because
    /// there is no way of detecting which of those two the other process sent.
    #[inline]
    pub fn ruid(&self) -> Option<uid_t> {
        self.0.ruid()
    }
    /// Returns the **closest thing to the real user ID** among what's stored in the credentials table. If a real UID is
    /// not present, the effective UID is returned instead.
    ///
    /// This method is intended to be used by daemons which need to verify user input for security purposes and would
    /// like to see past elevation via `setuid` programs if possible.
    pub fn best_effort_ruid(&self) -> uid_t {
        match (self.euid(), self.ruid()) {
            (Some(id), ..) | (None, Some(id)) => id,
            (None, None) => unreachable!(),
        }
    }
    /// Returns the **effective** group ID stored in the credentials table, or `None` if no such information is
    /// available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return the GID from `ucred` despite the Linux kernel allowing either the effective or the real GID
    /// to be sent.
    #[inline]
    pub fn egid(&self) -> Option<gid_t> {
        self.0.egid()
    }
    /// Returns the **real** group ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return `None`, even though `ucred` may contain either the effective or the real GID; this is because
    /// there is no way of detecting which of those two the other process sent.
    #[inline]
    pub fn rgid(&self) -> Option<gid_t> {
        self.0.rgid()
    }
    /// Returns the **closest thing to the real group ID** among what's stored in the credentials table. If a real GID
    /// is not present, the effective GID is returned instead.
    ///
    /// This method is intended to be used by daemons which need to verify user input for security purposes and would
    /// like to see past elevation via `setuid` programs if possible.
    pub fn best_effort_rgid(&self) -> gid_t {
        match (self.egid(), self.rgid()) {
            (Some(id), ..) | (None, Some(id)) => id,
            (None, None) => unreachable!(),
        }
    }
    /// Returns the process ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Privileged processes (those with `CAP_SYS_ADMIN`) may send any PID, as long as it belongs to an existing
    /// process. Note that actually relying on the kernel's check for PID validity is a possible [TOCTOU] bug.
    ///
    /// [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use
    #[inline]
    pub fn pid(&self) -> Option<pid_t> {
        self.0.pid()
    }
    /// Returns an iterator over the supplementary groups in the credentials table.
    ///
    /// The resulting iterator implements `ExactSizeIterator`, so the amount of supplementary groups can be queried
    /// without iterating through all via the `.len()` method.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Always empty.
    #[inline]
    pub fn groups(&self) -> Groups<'a> {
        Groups(self.0.groups())
    }
}

/// An iterator over supplementary groups stored in [`Credentials`].
///
/// # Platform-specific behavior
/// ## `ucred` (Linux)
/// Always empty.
pub struct Groups<'a>(GroupsImpl<'a>);
impl Iterator for Groups<'_> {
    type Item = gid_t;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl FusedIterator for Groups<'_> {}
impl ExactSizeIterator for Groups<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}
