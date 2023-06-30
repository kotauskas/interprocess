use crate::os::unix::{
    c_wrappers,
    udsocket::{
        cmsg::{ancillary::*, Cmsg},
        credentials::ucred::*,
    },
};
use libc::ucred;
use std::{mem::size_of, slice};

impl<'a> Credentials<'a> {
    pub const ANCTYPE: c_int = libc::SCM_CREDENTIALS;
    pub fn new_auto(ruid: bool, rgid: bool) -> Self {
        Self::Owned(ucred {
            pid: c_wrappers::get_pid(),
            uid: c_wrappers::get_uid(ruid),
            gid: c_wrappers::get_gid(rgid),
        })
    }
    pub fn new_borrowed(creds: &'a ucred) -> Self {
        Self::Borrowed(creds.as_ref())
    }
}

impl ToCmsg for Credentials<'_> {
    fn to_cmsg(&self) -> Cmsg<'_> {
        let st_bytes = unsafe {
            // SAFETY: well-initialized POD struct with #[repr(C)]
            slice::from_raw_parts(<*const _>::cast(self.as_ref()), size_of::<ucred>())
        };
        unsafe {
            // SAFETY: we've got checks to ensure that we're not using the wrong struct
            Cmsg::new(LEVEL, Self::ANCTYPE, st_bytes)
        }
    }
}

impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;
    type Context = (); // TODO

    fn try_parse(mut cmsg: Cmsg<'a>, _ctx: &()) -> ParseResult<'a, Self, SizeMismatch> {
        cmsg = check_level_and_type(cmsg, Self::ANCTYPE)?;
        cmsg = check_size(cmsg, size_of::<ucred>())?;

        let creds = unsafe {
            // SAFETY: POD
            &*cmsg.data().as_ptr().cast::<ucred>()
        }
        .as_ref();
        Ok(Self::Borrowed(creds))
    }
}
