use super::*;
use crate::os::unix::udsocket::cmsg::{ancillary::*, Cmsg};
use libc::{gid_t, pid_t, ucred, uid_t};
use std::{marker::PhantomData, mem::size_of, slice};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct Credentials<'a>(pub &'a ucred_packed);
impl<'a> Credentials<'a> {
    pub const TYPE: c_int = libc::SCM_CREDENTIALS;
    pub fn new(creds: &'a ucred) -> Self {
        Self(creds.as_ref())
    }
    pub fn euid(self) -> Option<uid_t> {
        Some(self.0.uid)
    }
    pub fn ruid(self) -> Option<uid_t> {
        None
    }
    pub fn egid(self) -> Option<gid_t> {
        Some(self.0.gid)
    }
    pub fn rgid(self) -> Option<gid_t> {
        None
    }
    pub fn pid(self) -> Option<pid_t> {
        Some(self.0.pid)
    }
    pub fn groups(&self) -> Groups<'a> {
        Groups(PhantomData)
    }
}

impl<'a> ToCmsg for Credentials<'a> {
    fn add_to_buffer(&self, add_fn: impl FnOnce(Cmsg<'_>)) {
        let st_bytes = unsafe {
            // SAFETY: well-initialized POD struct with #[repr(C)]
            slice::from_raw_parts(<*const _>::cast(self.0), size_of::<ucred>())
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

        let sz = cmsg.data().len();
        let expected = size_of::<ucred>();
        if sz != expected {
            return Err(MalformedPayload(SizeMismatch { expected, got: sz }).wrap(cmsg));
        }

        let creds = unsafe {
            // SAFETY: POD
            &*cmsg.data().as_ptr().cast::<ucred>()
        }
        .as_ref();
        Ok(Self(creds))
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
