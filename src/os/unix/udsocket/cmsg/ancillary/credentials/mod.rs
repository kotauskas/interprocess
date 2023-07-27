//! [`Credentials`] as an ancillary message type.
//!
//! In addition to what's re-exported from [`udsocket::credentials`](crate::os::unix::udsocket::credentials), this
//! module contains the context type required to deserialize ancillary messages of the [`Credentials`] variety.

#[cfg(uds_cmsgcred)]
mod freebsdlike;
#[cfg(uds_ucred)]
mod ucred;

use super::*;

pub use crate::os::unix::udsocket::credentials::*;

/// Functions for creating tables of credentials to be sent as ancillary messages.
impl<'a> Credentials<'a> {
    pub(super) const ANCTYPE: c_int = CredentialsImpl::ANCTYPE;
    /// Creates a `Credentials` ancillary data struct to be sent as a control message, storing it by value. This allows
    /// for impersonation of other processes, users and groups given sufficient privileges, and is not strictly
    /// necessary for the other end to receive this type of ancillary data.
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
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn from_ucred(creds: libc::ucred) -> Self {
        Self(CredentialsImpl::Owned(creds))
    }
    /// Creates a `Credentials` ancillary data struct to be sent as a control message from a borrow. This allows for
    /// impersonation of other processes, users and groups given sufficient privileges, and is not strictly necessary
    /// for the other end to receive this type of ancillary data.
    ///
    /// If the given `ucred` structure is filled out incorrectly, sending this message will fail with an error. See the
    /// documentation on [`from_ucred()`](Self::from_ucred) for more details.
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
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
            target_os = "redox",
            target_os = "android",
            target_os = "fuchsia",
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
    #[cfg_attr( // uds_cmsgcred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
        )))
    )]
    #[cfg(uds_cmsgcred)]
    #[inline]
    pub fn sendable_cmsgcred() -> Self {
        Self(CredentialsImpl::Cmsgcred(freebsdlike::ZEROED_CMSGCRED.as_ref()))
    }
}

/// Sending will set the credentials that the receieving end will read if they have credentials passing enabled.
///
/// The kernel checks the contents of those ancillary messages to make sure that unprivileged processes can't
/// impersonate anyone, allowing for secure authentication. For this reason, not all values of `Credentials` created for
/// sending can be sent without errors. See the associated functions that create values of `Credentials` without parsing
/// them for more information on the sorts of invariants which must be upheld.
///
/// It's impossible to cause undefined behavior in sound code by sending wrong values, and the send operation will
/// simply return an error.
#[cfg_attr( // uds_credentials template
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "redox",
        target_os = "android",
        target_os = "fuchsia",
        target_os = "freebsd",
        target_os = "dragonfly",
    )))
)]
impl ToCmsg for Credentials<'_> {
    #[inline]
    fn to_cmsg(&self) -> Cmsg<'_> {
        self.0.to_cmsg()
    }
}
#[cfg_attr( // uds_credentials template
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "redox",
        target_os = "android",
        target_os = "fuchsia",
        target_os = "freebsd",
        target_os = "dragonfly",
    )))
)]
impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;
    #[inline]
    fn try_parse(cmsg: Cmsg<'a>) -> ParseResult<'a, Self, Self::MalformedPayloadError> {
        CredentialsImpl::try_parse(cmsg).map(Self)
    }
}
