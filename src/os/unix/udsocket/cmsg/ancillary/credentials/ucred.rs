use super::*;
use crate::os::unix::{
    c_wrappers,
    udsocket::cmsg::{ancillary::*, Cmsg},
};
use libc::{gid_t, pid_t, ucred, uid_t};
use std::{marker::PhantomData, mem::size_of, slice};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Credentials<'a> {
    Borrowed(&'a ucred_packed),
    Owned(ucred),
}
impl<'a> Credentials<'a> {
    pub const TYPE: c_int = libc::SCM_CREDENTIALS;
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
    pub fn euid(self) -> Option<uid_t> {
        Some(self.as_ref().uid)
    }
    pub fn ruid(self) -> Option<uid_t> {
        None
    }
    pub fn egid(self) -> Option<gid_t> {
        Some(self.as_ref().gid)
    }
    pub fn rgid(self) -> Option<gid_t> {
        None
    }
    pub fn pid(self) -> Option<pid_t> {
        Some(self.as_ref().pid)
    }
    pub fn groups(&self) -> Groups<'a> {
        Groups(PhantomData)
    }
}
impl AsRef<ucred_packed> for Credentials<'_> {
    #[inline]
    fn as_ref(&self) -> &ucred_packed {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(o) => o.as_ref(),
        }
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
            Cmsg::new(LEVEL, Self::TYPE, st_bytes)
        }
    }
}

impl<'a> FromCmsg<'a> for Credentials<'a> {
    type MalformedPayloadError = SizeMismatch;

    fn try_parse(mut cmsg: Cmsg<'a>) -> ParseResult<'a, Self, SizeMismatch> {
        cmsg = check_level_and_type(cmsg, Self::TYPE)?;
        cmsg = check_size(cmsg, size_of::<ucred>())?;

        let creds = unsafe {
            // SAFETY: POD
            &*cmsg.data().as_ptr().cast::<ucred>()
        }
        .as_ref();
        Ok(Self::Borrowed(creds))
    }
}

#[cfg(uds_ucred)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct ucred_packed {
    pid: pid_t,
    uid: uid_t,
    gid: gid_t,
}
impl AsRef<ucred_packed> for ucred {
    fn as_ref(&self) -> &ucred_packed {
        const _: () = {
            if size_of::<ucred_packed>() != size_of::<ucred>() {
                panic!("size of `ucred_packed` did not match that of `ucred`");
            }
        };
        unsafe {
            // SAFETY: the two types have the same layout, save for stricter padding of the input
            &*<*const _>::cast(self)
        }
    }
}

#[derive(Clone, Default, Debug)]
pub(super) struct Groups<'a>(PhantomData<&'a ucred>);
impl Iterator for Groups<'_> {
    type Item = gid_t;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}
impl ExactSizeIterator for Groups<'_> {
    #[inline]
    fn len(&self) -> usize {
        0
    }
}
