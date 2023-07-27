use crate::os::unix::{
    udsocket::{
        cmsg::{ancillary::*, Cmsg},
        credentials::freebsdlike::*,
    },
    unixprelude::*,
};
use libc::{cmsgcred, SCM_CREDS};
#[cfg(uds_sockcred2)]
use libc::{sockcred2, SCM_CREDS2};
use std::{mem::size_of, slice};

impl Credentials<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Cmsgcred(..) => size_of::<cmsgcred>(),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => unsafe { libc::SOCKCRED2SIZE(c.sc_ngroups as _) },
        }
    }
}

impl<'a> ToCmsg for Credentials<'a> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        let (st_bytes, anctype) = unsafe {
            let (ptr, anctype) = match self {
                Credentials::Cmsgcred(c) => (<*const _>::cast(c), SCM_CREDS),
                #[cfg(uds_sockcred2)]
                Self::Sockcred2(c) => (<*const _>::cast(c), SCM_CREDS2),
            };
            // SAFETY: well-initialized POD struct with #[repr(C)]
            (slice::from_raw_parts(ptr, self.len()), anctype)
        };
        unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, anctype, st_bytes)
        }
    }
}

impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;

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

pub(super) static ZEROED_CMSGCRED: cmsgcred = cmsgcred {
    cmcred_pid: 0,
    cmcred_uid: 0,
    cmcred_euid: 0,
    cmcred_gid: 0,
    cmcred_ngroups: 0,
    cmcred_groups: [0; libc::CMGROUP_MAX],
};
