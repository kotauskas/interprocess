use crate::os::unix::{
    udsocket::{
        cmsg::{ancillary::*, Cmsg},
        credentials::freebsdlike::*,
    },
    unixprelude::*,
};
use libc::cmsgcred;
use std::{mem::size_of, slice};

impl Credentials<'_> {
    pub const ANCTYPE: c_int = libc::SCM_CREDS;
    fn len(&self) -> usize {
        match self {
            Self::Cmsgcred(..) => size_of::<cmsgcred>(),
        }
    }
}

impl<'a> ToCmsg for Credentials<'a> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        let st_bytes = unsafe {
            let ptr = match self {
                Credentials::Cmsgcred(c) => <*const _>::cast(c),
            };
            // SAFETY: well-initialized POD struct with #[repr(C)]
            slice::from_raw_parts(ptr, self.len())
        };
        unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, Self::ANCTYPE, st_bytes)
        }
    }
}

impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;

    fn try_parse(mut cmsg: Cmsg<'a>) -> ParseResult<'a, Self, SizeMismatch> {
        cmsg = check_level_and_type(cmsg, Self::ANCTYPE)?;
        unsafe { into_fixed_size_contents::<cmsgcred_packed>(cmsg) }.map(Self::Cmsgcred)
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
