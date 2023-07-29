//! [`Credentials`] as an ancillary message type.
//!
//! In addition to what's re-exported from [`udsocket::credentials`](crate::os::unix::udsocket::credentials), this
//! module contains the context type required to deserialize ancillary messages of the [`Credentials`] variety.

pub use crate::os::unix::udsocket::credentials::*;

use super::*;
#[cfg(uds_cmsgcred)]
use libc::cmsgcred;
#[cfg(uds_sockcred2)]
use libc::sockcred2;
#[cfg(uds_ucred)]
use libc::ucred;
use std::{mem::size_of, slice};

/// Functions for creating tables of credentials to be sent as ancillary messages.
impl<'a> Credentials<'a> {
    pub(super) const ANCTYPE1: c_int = {
        #[cfg(uds_ucred)]
        {
            libc::SCM_CREDENTIALS
        }
        #[cfg(uds_cmsgcred)]
        {
            libc::SCM_CREDS
        }
    };
    #[cfg(uds_sockcred2)]
    pub(super) const ANCTYPE2: c_int = libc::SCM_CREDS2;
    /// The smallest possible ancillary *payload size* of the largest supported credentials structure on the current
    /// platform, as a [`c_uint`].
    ///
    /// You can use [`Cmsg::cmsg_len_for_payload_size()`](crate::os::unix::udsocket::cmsg::Cmsg) to calculate the
    /// smallest compatible buffer size.
    ///
    /// Note that this does not actually guarantee reception of certain types ancillary messages, with `sockcred2` on
    /// FreeBSD being the worst offender, since their dynamically-sized nature is often ignored by the code in the OS
    /// that handles truncation. You must always check the truncation flag be sure.
    pub const MIN_ANCILLARY_SIZE: c_uint = {
        #[cfg(uds_ucred)]
        {
            size_of::<ucred>()
        }
        #[cfg(uds_cmsgcred)]
        {
            size_of::<cmsgcred>()
        }
    } as c_uint;
    /// Creates a `Credentials` ancillary data struct to be sent as a control message, storing it by value. This allows
    /// for impersonation of other processes, users and groups given sufficient privileges, and is not strictly
    /// necessary for the other end to receive this type of ancillary data.
    ///
    /// # Validity
    /// If the given `ucred` structure is filled out incorrectly, sending this message will fail with an error. The
    /// requirements are as follows:
    /// - ***`pid`*** must be the PID of the sending process, unless the it has the `CAP_SYS_ADMIN` capability, in
    /// which case any valid PID can be specified. Note that not even privileged processes may specify PIDs of
    /// nonexistent processes.
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
    pub fn from_ucred(creds: ucred) -> Self {
        Self(CredentialsInner::Ucred(creds))
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
    pub fn from_ucred_ref(creds: &'a ucred) -> Self {
        Self(CredentialsInner::AncUcred(creds.as_ref()))
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
        use super::super::super::c_wrappers;
        Self(CredentialsInner::Ucred(ucred {
            pid: c_wrappers::get_pid(),
            uid: c_wrappers::get_uid(ruid),
            gid: c_wrappers::get_gid(rgid),
        }))
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
        Self(CredentialsImpl::Cmsgcred(ZEROED_CMSGCRED.as_ref()))
    }

    fn tocmslice(&self) -> &[u8] {
        #[cfg(uds_ucred)]
        {
            let ucp = match self.0 {
                CredentialsInner::AncUcred(c) => c,
                CredentialsInner::Ucred(ref c) => c.as_ref(),
            };
            unsafe {
                // SAFETY: well-initialized POD struct with #[repr(C)]
                slice::from_raw_parts(<*const _>::cast(ucp), size_of::<ucred>())
            }
        }
        #[cfg(uds_cmsgcred)]
        #[allow(unreachable_patterns)]
        {
            unsafe {
                let ptr = match self.0 {
                    CredentialsInner::Cmsgcred(c) => <*const _>::cast(c),
                    els => panic!("not a sendable credentials structure"),
                };
                // SAFETY: well-initialized POD struct with #[repr(C)]
                slice::from_raw_parts(ptr, size_of::<cmsgcred>())
            }
        }
    }
}

/// Sending will set the credentials that the receieving end will read.
///
/// The kernel checks the contents of those ancillary messages to make sure that unprivileged processes can't
/// impersonate anyone, allowing for secure authentication. For this reason, not all values of `Credentials` created for
/// sending can be sent without errors. See the associated functions that create values of `Credentials` without parsing
/// them for more information on the sorts of invariants which must be upheld.
///
/// It's impossible to cause undefined behavior in sound code by sending wrong values, and the send operation will
/// simply return an error.
///
/// # Panics
/// Only `ucred` (Linux) and `cmsgcred` (FreeBSD, DragonFly BSD) support this functionality. Attempting to serialize
/// other types of structures (possible on FreeBSD in the case of `xucred` and `sockcred2`) will cause a panic in
/// `.to_cmsg()`.
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
        unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, Self::ANCTYPE1, self.tocmslice())
        }
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
    #[cfg(uds_ucred)]
    #[inline]
    fn try_parse(mut cmsg: Cmsg<'a>) -> ParseResult<'a, Self, Self::MalformedPayloadError> {
        cmsg = check_level_and_type(cmsg, Self::ANCTYPE1)?;
        unsafe { into_fixed_size_contents::<ucred_packed>(cmsg) }
            .map(CredentialsInner::AncUcred)
            .map(Self)
    }
    #[cfg(uds_cmsgcred)]
    fn try_parse(mut cmsg: Cmsg<'a>) -> ParseResult<'a, Self, SizeMismatch> {
        cmsg = check_level(cmsg)?;
        let expected = if !cfg!(uds_sockcred2) { Some(SCM_CREDS) } else { None };
        match cmsg.cmsg_type() {
            SCM_CREDS => unsafe { into_fixed_size_contents::<cmsgcred_packed>(cmsg) }.map(Self::Cmsgcred),
            #[cfg(uds_sockcred2)]
            SCM_CREDS2 => {
                let min_expected = size_of::<sockcred2>();
                let len = cmsg.data().len();
                if len < min_expected {
                    // If this is false, we can't even do the reinterpret and figure out the number
                    // of supplementary groups; prioritize formal soundness over error reporting
                    // precision in this niche case and claim to expect the base size of sockcred.
                    return Err(ParseErrorKind::MalformedPayload(SizeMismatch {
                        expected: min_expected,
                        got: len,
                    })
                    .wrap(cmsg));
                }

                let creds = unsafe {
                    // SAFETY: POD
                    &*cmsg.data().as_ptr().cast::<sockcred2>()
                };

                let expected = unsafe { libc::SOCKCRED2SIZE(creds.sc_ngroups as _) };
                // Be nice on the alignment here.
                if len < expected {
                    // The rest of the size error reporting process happens here.
                    return Err(ParseErrorKind::MalformedPayload(SizeMismatch { expected, got: len }).wrap(cmsg));
                }

                Ok(Self::Sockcred2(creds.as_ref()))
            }
            els => Err(ParseErrorKind::WrongType { expected, got: els }.wrap(cmsg)),
        }
    }
}

#[cfg(uds_cmsgcred)]
pub(super) static ZEROED_CMSGCRED: cmsgcred = cmsgcred {
    cmcred_pid: 0,
    cmcred_uid: 0,
    cmcred_euid: 0,
    cmcred_gid: 0,
    cmcred_ngroups: 0,
    cmcred_groups: [0; libc::CMGROUP_MAX],
};
