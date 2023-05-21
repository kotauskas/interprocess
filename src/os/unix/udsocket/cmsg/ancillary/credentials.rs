//! [`Credentials`] and associated helper types.

// FIXME uds_sockcred is disabled in build.rs for reasons outlined there.

use super::*;
use libc::{c_int, gid_t, pid_t, uid_t};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    // iter::FusedIterator,
    mem::size_of,
    slice,
};

/// Ancillary data message that allows receiving the credentials of the peer process and, on some systems, setting the contents of this ancillary message that the other process will receive.
///
/// To receive this message, the `SO_PASSCRED` socket option must be enabled. After it's enabled, every receive operation that provides an ancillary data buffer will receive an instance of this message.
#[derive(Copy, Clone, Debug, Eq)]
pub struct Credentials<'a>(&'a CredType);
impl<'a> Credentials<'a> {
    pub(super) const TYPE: c_int = {
        #[cfg(uds_ucred)]
        {
            libc::SCM_CREDENTIALS
        }
        #[cfg(uds_sockcred)]
        {
            libc::SCM_CREDS
        }
    };
    /// Creates a `Credentials` ancillary data struct to be sent as a control message. This allows for impersonation of other processes, users and groups given sufficient privileges, and is not necessary for the other end to recieve this type of ancillary data. Only available on `ucred` platforms.
    #[cfg_attr( // uds_ucred template
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
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    #[inline]
    pub fn new_sendable(creds: &'a libc::ucred) -> Self {
        Self(unsafe {
            // SAFETY: CredType is layout-compatible and less strictly aligned
            &*(creds as *const libc::ucred).cast::<CredType>()
        })
    }
    /// Returns the effective user ID stored in the credentials table, or `None` if no such information is available.
    #[inline]
    pub fn effective_uid(&self) -> Option<uid_t> {
        #[cfg(uds_ucred)]
        {
            None
        }
        #[cfg(uds_sockcred)]
        {
            Some(self.0.sc_euid)
        }
    }
    /// Returns the real user ID stored in the credentials table, or `None` if no such information is available.
    #[inline]
    pub fn real_uid(&self) -> Option<uid_t> {
        #[cfg(uds_ucred)]
        {
            Some(self.0.uid)
        }
        #[cfg(uds_sockcred)]
        {
            Some(self.0.sc_euid)
        }
    }
    /// Returns the effective group ID stored in the credentials table, or `None` if no such information is available.
    #[inline]
    pub fn effective_gid(&self) -> Option<gid_t> {
        #[cfg(uds_ucred)]
        {
            None
        }
        #[cfg(uds_sockcred)]
        {
            Some(self.0.sc_egid)
        }
    }
    /// Returns the real group ID stored in the credentials table, or `None` if no such information is available.
    #[inline]
    pub fn real_gid(&self) -> Option<gid_t> {
        #[cfg(uds_ucred)]
        {
            Some(self.0.gid)
        }
        #[cfg(uds_sockcred)]
        {
            Some(self.0.sc_egid)
        }
    }
    /// Returns the process ID stored in the credentials table, or `None` if no such information is available.
    #[inline]
    pub fn pid(&self) -> Option<pid_t> {
        #[cfg(uds_ucred)]
        {
            Some(self.0.pid)
        }
        #[cfg(uds_sockcred)]
        {
            None
        }
    }
    /// Returns an iterator over the supplementary groups in the credentials table. Only available on `sockcred` platforms.
    #[cfg_attr(feature = "doc_cfg", doc(cfg(sockcred)))]
    #[cfg(uds_sockcred)]
    fn groups(&self) -> Groups<'a> {
        Groups {
            cur: (&self.0.sc_groups as *const [gid_t; 1]).cast::<u8>(),
            i: 0,
            cred: self,
        }
    }
    /*
    fn n_sgroups(&self) -> c_int {
        #[cfg(uds_ucred)]
        {
            0
        }
        #[cfg(uds_sockcred)]
        {
            self.0.sc_ngroups
        }
    }
    */
}
impl PartialEq for Credentials<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.0 != other.0 {
            return false;
        }
        #[cfg(uds_sockcred)]
        {
            self.0.groups().eq(other.groups())
        }
        #[cfg(uds_ucred)]
        {
            true
        }
    }
}
/// Sending will set the credentials that the receieving end will read with `SO_PASSCRED`. Only available on `ucred` systems.
#[cfg_attr( // uds_ucred template
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
        target_os = "redox"
    )))
)]
#[cfg(uds_ucred)]
impl<'a> ToCmsg for Credentials<'a> {
    fn add_to_buffer(&self, add_fn: impl FnOnce(Cmsg<'_>)) {
        let st_bytes = unsafe {
            // SAFETY: well-initialized POD struct with #[repr(C)]
            slice::from_raw_parts((self.0 as *const CredType).cast::<u8>(), size_of::<CredType>())
        };
        let cmsg = unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, Self::TYPE, st_bytes)
        };
        add_fn(cmsg);
    }
}
impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;

    fn try_parse(cmsg: Cmsg<'a>) -> ParseResult<'a, Self, SizeMismatch> {
        use ParseErrorKind::*;
        let (lvl, ty) = (cmsg.cmsg_level(), cmsg.cmsg_type());
        if lvl != LEVEL {
            return Err(WrongLevel {
                expected: Some(LEVEL),
                got: lvl,
            }
            .wrap(cmsg));
        }
        if ty != Self::TYPE {
            return Err(WrongType {
                expected: Some(Self::TYPE),
                got: ty,
            }
            .wrap(cmsg));
        }
        // sockcred is CDST with the fucking supplementary group info
        if cfg!(uds_ucred) {
            let sz = cmsg.data().len();
            let expected = size_of::<CredType>();
            if sz != expected {
                return Err(MalformedPayload(SizeMismatch { expected, got: sz }).wrap(cmsg));
            }
        }

        let creds = unsafe {
            // SAFETY: we just checked for the size match; if it does, a packed #[repr(C)] POD struct could only
            // possibly receieve wrong field values in this sort of reinterpret
            &*cmsg.data().as_ptr().cast::<CredType>()
        };
        Ok(Self(creds))
    }
}
/// A [`MalformedPayload`](ParseErrorKind::MalformedPayload) error indicating that the ancillary message size dosen't match that of the platform-specific credentials structure.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SizeMismatch {
    expected: usize,
    got: usize,
}
impl Display for SizeMismatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { expected, got } = self;
        write!(f, "ancillary payload size mismatch (expected {expected}, got {got})")
    }
}
impl Error for SizeMismatch {}

/*
/// An iterator over supplementary groups of [`Credentials`].
///
/// Unobtainable and no-op on `ucred` platforms.
#[derive(Clone, Debug)]
pub struct Groups<'a> {
    cur: *const [u8; size_of::<gid_t>()],
    i: c_int,
    cred: Credentials<'a>,
}
impl Iterator for Groups<'_> {
    type Item = gid_t;

    fn next(&mut self) -> Option<Self::Item> {
        self.i += 1;
        if self.i > self.cred.n_sgroups() {
            return None;
        }
        let gid_bytes = unsafe { *self.cur };
        self.cur = self.cur.wrapping_add(1);
        Some(gid_t::from_ne_bytes(gid_bytes))
    }
}
impl FusedIterator for Groups<'_> {}
*/

#[cfg(uds_ucred)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct CredType {
    pid: pid_t,
    uid: uid_t,
    gid: gid_t,
}
#[cfg(uds_ucred)]
static _CHK_UCRED: () = {
    // Validates that ucred is present and the build script isn't lying
    let _ = libc::ucred { pid: 0, uid: 0, gid: 0 };
};

/*
#[cfg(uds_sockcred)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct CredType {
    sc_uid: uid_t,
    sc_euid: uid_t,
    sc_gid: gid_t,
    sc_egid: gid_t,
    sc_ngroups: c_int,
    sc_groups: [gid_t; 1],
}
#[cfg(uds_sockcred)]
static _CHK_SOCKCRED: () = {
    // Validates that sockcred is present and the build script isn't lying
    let _ = libc::sockcred {
        sc_uid: 0,
        sc_euid: 0,
        sc_gid: 0,
        sc_egid: 0,
        sc_ngroups: 0,
        sc_groups: [0],
    };
};
*/

#[cfg(not(any(uds_ucred, uds_sockcred)))]
type CredType = ();
