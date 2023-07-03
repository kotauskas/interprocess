use super::Context;
use crate::os::unix::{
    udsocket::{
        cmsg::{ancillary::*, context::Collector, Cmsg},
        credentials::freebsdlike::*,
    },
    unixprelude::*,
};
use libc::cmsgcred;
use std::{mem::size_of, slice};

#[cfg(uds_sockcred)]
use {crate::os::unix::udsocket::c_wrappers, libc::sockcred};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(super) struct CredsOptContext {
    local_creds: bool,
}
impl Collector for CredsOptContext {
    fn pre_op_collect(&mut self, socket: BorrowedFd<'_>) {
        #[cfg(uds_sockcred)]
        if let Ok(val) = c_wrappers::get_local_creds(socket) {
            self.local_creds = val;
        }
    }
}

impl Credentials<'_> {
    pub const ANCTYPE: c_int = libc::SCM_CREDS;
}

impl<'a> ToCmsg for Credentials<'a> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        let st_bytes = unsafe {
            let (ptr, len) = match self {
                Credentials::Cmsgcred(c) => (<*const _>::cast(c), size_of::<cmsgcred>()),
                #[cfg(uds_sockcred)]
                Credentials::Sockcred(c) => (<*const _>::cast(c), libc::SOCKCREDSIZE(c.sc_ngroups as _)),
            };
            // SAFETY: well-initialized POD struct with #[repr(C)]
            slice::from_raw_parts(ptr, len)
        };
        unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, Self::ANCTYPE, st_bytes)
        }
    }
}

impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;
    type Context = Context;

    fn try_parse(mut cmsg: Cmsg<'a>, ctx: &Context) -> ParseResult<'a, Self, SizeMismatch> {
        cmsg = check_level_and_type(cmsg, Self::ANCTYPE)?;
        if ctx.platform.local_creds {
            // LOCAL_CREDS is set
            #[cfg(not(uds_sockcred))]
            {
                unreachable!("corrupted context (LOCAL_CREDS reported set on a platform that doesn't support it)")
            }
            #[cfg(uds_sockcred)]
            {
                let min_expected = size_of::<sockcred>();
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
                    &*cmsg.data().as_ptr().cast::<sockcred>()
                };

                let expected = unsafe { libc::SOCKCREDSIZE(creds.sc_ngroups as _) };
                if len != expected {
                    // The rest of the size error reporting process happens here.
                    return Err(ParseErrorKind::MalformedPayload(SizeMismatch {
                        expected: min_expected,
                        got: len,
                    })
                    .wrap(cmsg));
                }

                Ok(Self::Sockcred(creds.as_ref()))
            }
        } else {
            unsafe { into_fixed_size_contents::<cmsgcred_packed>(cmsg) }.map(Self::Cmsgcred)
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
#[cfg(uds_sockcred)]
pub(super) static ZEROED_SOCKCRED: sockcred = sockcred {
    sc_uid: 0,
    sc_euid: 0,
    sc_gid: 0,
    sc_egid: 0,
    sc_ngroups: 0,
    sc_groups: [0],
};
