//! [`Credentials`] and associated helper types.

// FIXME uds_sockcred is disabled in build.rs for reasons outlined there.

#[cfg(uds_cmsgcred)]
mod freebsdlike;
#[cfg(uds_ucred)]
mod ucred;
cfg_if::cfg_if! {
    if #[cfg(uds_ucred)] {
        use ucred::{Credentials as CredentialsImpl, Groups as GroupsImpl};
    } else if #[cfg(uds_cmsgcred)] {
        use freebsdlike::{Credentials as CredentialsImpl, Groups as GroupsImpl};
    }
}

use super::*;
use libc::{c_int, gid_t, pid_t, uid_t};
use std::iter::FusedIterator;

/// Ancillary data message that allows receiving the credentials of the peer process and, on some systems, setting the
/// contents of this ancillary message that the other process will receive.
///
/// To receive this message, the `SO_PASSCRED` socket option must be enabled. After it's enabled, every receive
/// operation that provides an ancillary data buffer will receive an instance of this message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
// TODO move out of ancillary and reexport here instead
pub struct Credentials<'a>(CredentialsImpl<'a>);
impl<'a> Credentials<'a> {
    pub(super) const TYPE: c_int = CredentialsImpl::TYPE;
    /// Creates a `Credentials` ancillary data struct to be sent as a control message, storing it by value. This allows
    /// for impersonation of other processes, users and groups given sufficient privileges, and is not strictly
    /// necessary for the other end to recieve this type of ancillary data.
    ///
    /// # Validity
    /// If the given `ucred` structure is filled out incorrectly, sending this message will fail with an error. The
    /// requirements are as follows:
    /// - ***`pid`*** must be the PID of the sending process, unless the it has the `CAP_SYS_ADMIN` capability, in which case
    /// any valid PID can be specified. Note that not even privileged processes may specify PIDs of nonexistent
    /// processes.
    /// - ***`uid`*** must be the sender's real UID, effective UID or saved set-user-ID, unless it has the `CAP_SETUID`
    /// capability, in which case any valid user ID may be specified.
    /// - ***`gid`*** must be the sender's real GID, effective GID or saved set-user-ID, unless it has the `CAP_SETGID`
    /// capability, in which case any valid group ID may be specified.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn from_ucred(creds: libc::ucred) -> Self {
        Self(CredentialsImpl::Owned(creds))
    }
    /// Creates a `Credentials` ancillary data struct to be sent as a control message from a borrow. This allows for
    /// impersonation of other processes, users and groups given sufficient privileges, and is not strictly necessary
    /// for the other end to recieve this type of ancillary data.
    ///
    /// If the given `ucred` structure is filled out incorrectly, sending this message will fail with an error. See the
    /// documentation on [`from_ucred()`](Self::from_ucred) for more details.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn from_ucred_ref(creds: &'a libc::ucred) -> Self {
        Self(CredentialsImpl::new_borrowed(creds))
    }
    /// Creates a `Credentials` ancillary data struct to be sent as a control message by automatically filling in the
    /// underlying `ucred` structure with the PID, effective UID and effective GID of the calling process. The two
    /// boolean paramaters allow the real UID and real GID to be used instead.
    ///
    /// Sending the message from a `fork`ed process will fail, unless it has the `CAP_SYS_ADMIN` capability. This is
    /// because the PID in the structure will still be that of the parent process.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn new_ucred(ruid: bool, rgid: bool) -> Self {
        Self(CredentialsImpl::new_auto(ruid, rgid))
    }
    /// Creates a `Credentials` ancillary data struct of the `cmsgcred` variety to be sent as a control message. The
    /// underlying value is zeroed out and automatically filled in by the kernel.
    ///
    /// The receiver will be able to read the sender's PID, real and effective UID, real GID and up to `CMGROUP_MAX`
    /// (16) supplemental groups.
    ///
    /// The `LOCAL_CREDS` option must be *disabled* for this ancillary data struct to be sent.
    #[cfg_attr( // uds_cmsgcred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly"
        )))
    )]
    #[cfg(uds_cmsgcred)]
    #[inline]
    pub fn sendable_cmsgcred() -> Self {
        Self(CredentialsImpl::Cmsgcred(freebsdlike::ZEROED_CMSGCRED))
    }
    /// Creates a `Credentials` ancillary data struct of the `sockcred` variety to be sent as a control message. The
    /// underlying value is zeroed out and automatically filled in by the kernel.
    ///
    /// The receiver will be able to read the sender's real and effective UID, real and effective GID and an unspecified
    /// amount of supplemental groups. As per the [FreeBSD manual page for `unix(4)`][mp], the supplemental group list
    /// is currently truncated to `CMGROUP_MAX` (16) entries.
    ///
    /// The `LOCAL_CREDS` option must be *enabled* for this ancillary data struct to be sent.
    ///
    /// [mp]: https://man.freebsd.org/cgi/man.cgi?query=unix&sektion=0&manpath=FreeBSD+13.2-RELEASE+and+Ports
    #[cfg_attr( // uds_cmsgcred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly"
        )))
    )]
    #[cfg(uds_sockcred)]
    #[inline]
    pub fn sendable_sockcred() -> Self {
        Self(CredentialsImpl::Sockcred(freebsdlike::ZEROED_SOCKCRED))
    }
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

/// Sending will set the credentials that the receieving end will read with `SO_PASSCRED`.
///
/// The kernel checks the contents of those ancillary messages to make sure that unprivileged processes can't
/// impersonate anyone, allowing for secure authentication. For this reason, not all values of `Credentials` created for
/// sending can be sent without errors. See the associated functions that create values of `Credentials` without parsing
/// them for more information on the sorts of invariants which must be upheld.
///
/// It's impossible to cause undefined behavior in sound code by sending wrong values, and the send operation will
/// simply return an error.
// TODO platform-specific?
impl ToCmsg for Credentials<'_> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        self.0.to_cmsg()
    }
}
impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;
    #[inline]
    fn try_parse(cmsg: Cmsg<'a>) -> ParseResult<'a, Self, Self::MalformedPayloadError> {
        CredentialsImpl::try_parse(cmsg).map(Self)
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
