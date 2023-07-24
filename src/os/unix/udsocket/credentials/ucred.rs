use libc::{gid_t, pid_t, ucred, uid_t};
use std::{marker::PhantomData, mem::size_of};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Credentials<'a> {
    Borrowed(&'a ucred_packed),
    Owned(ucred),
}
impl<'a> Credentials<'a> {
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

#[cfg(uds_ucred)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub(crate) struct ucred_packed {
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
pub(crate) struct Groups<'a>(PhantomData<&'a ucred>);
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
