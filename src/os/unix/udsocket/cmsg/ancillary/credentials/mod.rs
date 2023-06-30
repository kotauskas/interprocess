//! [`Credentials`] as an ancillary message type.
//!
//! In addition to what's re-exported from [`udsocket::credentials`](crate::os::unix::udsocket::credentials), this
//! module contains the context type required to deserialize ancillary messages of the [`Credentials`] variety.

#[cfg(uds_cmsgcred)]
mod freebsdlike;
#[cfg(uds_ucred)]
mod ucred;

cfg_if::cfg_if! {
    if #[cfg(uds_sockcred)] {
        use freebsdlike::CredsOptContext as PlatformContext;
    } else {
        use crate::os::unix::udsocket::cmsg::context::DummyCollector as PlatformContext;
    }
}

use super::*;
use crate::os::unix::{udsocket::cmsg::context::Collector, unixprelude::*};
use std::cell::Cell;

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
    /// for the other end to receive this type of ancillary data.
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
}

/// A context [`Collector`] required for parsing of [`Credentials`].
///
/// Allowing this collector to collect the necessary context is mandatory on all platforms on which `Credentials`
/// exists, but it only serves a purpose on FreeBSD: obtaining the value of the `LOCAL_CREDS` socket option to
/// disambiguate `cmsgcred` and `sockcred`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Context {
    fresh: Cell<bool>,
    platform: PlatformContext,
}
impl Collector for Context {
    fn pre_op_collect(&mut self, socket: BorrowedFd<'_>) {
        self.platform.pre_op_collect(socket);
    }
    fn post_op_collect(&mut self, socket: BorrowedFd<'_>, msghdr_flags: c_int) {
        self.fresh.set(true);
        self.platform.post_op_collect(socket, msghdr_flags);
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
#[cfg_attr(
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox",
        target_os = "freebsd",
        target_os = "dragonfly",
    )))
)]
impl ToCmsg for Credentials<'_> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        self.0.to_cmsg()
    }
}
#[cfg_attr(
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox",
        target_os = "freebsd",
        target_os = "dragonfly",
    )))
)]
impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;
    type Context = Context;
    #[inline]
    fn try_parse(cmsg: Cmsg<'a>, ctx: &Self::Context) -> ParseResult<'a, Self, Self::MalformedPayloadError> {
        if !ctx.fresh.get() {
            // Give me downstream portability or give me death!
            return Err(ParseErrorKind::InsufficientContext.wrap(cmsg));
        }
        ctx.fresh.set(false);
        CredentialsImpl::try_parse(cmsg, ctx).map(Self)
    }
}
