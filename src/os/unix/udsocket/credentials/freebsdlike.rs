use libc::{c_int, c_short, cmsgcred, gid_t, pid_t, uid_t};
use std::{cmp::min, marker::PhantomData, mem::size_of, ptr::addr_of};
use to_method::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Credentials<'a>(&'a cmsgcred_packed);
impl<'a> Credentials<'a> {
    pub fn euid(self) -> Option<uid_t> {
        Some(c.cmcred_euid)
    }
    pub fn ruid(self) -> Option<uid_t> {
        Some(c.cmcred_uid)
    }
    pub fn egid(self) -> Option<gid_t> {
        None
    }
    pub fn rgid(self) -> Option<gid_t> {
        Some(c.cmcred_gid)
    }
    pub fn pid(self) -> Option<pid_t> {
        c.cmcred_pid
    }
    pub fn groups(&self) -> Groups<'a> {
        let n_groups = min(c.cmcred_ngroups, libc::CMGROUP_MAX as _).try_to::<usize>().unwrap();
        let cur = <*const _>::cast::<gid_packed>(addr_of!(c.cmcred_groups));
        let end = unsafe {
            // SAFETY: this puts us one byte past the last one
            cur.add(n_groups)
        };
        Groups {
            cur,
            end,
            _phantom: PhantomData,
        }
    }
}

#[allow(non_camel_case_types)]
type gid_packed = [u8; size_of::<gid_t>()];

#[derive(Clone, Debug)]
pub struct Groups<'a> {
    cur: *const gid_packed,
    end: *const gid_packed,
    _phantom: PhantomData<Credentials<'a>>,
}
impl Iterator for Groups<'_> {
    type Item = gid_t;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cur >= self.end {
            return None;
        }
        let gid_bytes = unsafe { *self.cur };
        self.cur = unsafe {
            // SAFETY: furthest this can go is one byte past the end, which is allowed
            self.cur.add(1)
        };
        Some(gid_t::from_ne_bytes(gid_bytes))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
impl ExactSizeIterator for Groups<'_> {
    #[inline]
    fn len(&self) -> usize {
        unsafe { self.end.offset_from(self.cur) as usize }
    }
}

// The two below are pub(crate) solely to allow the enum to have them

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub(crate) struct cmsgcred_packed {
    pub cmcred_pid: pid_t,
    pub cmcred_uid: uid_t,
    pub cmcred_euid: uid_t,
    pub cmcred_gid: gid_t,
    pub cmcred_ngroups: c_short,
    pub __pad0: c_short,
    pub cmcred_groups: [gid_t; 16],
}
impl AsRef<cmsgcred_packed> for cmsgcred {
    fn as_ref(&self) -> &cmsgcred_packed {
        const _: () = {
            if size_of::<cmsgcred_packed>() != size_of::<cmsgcred>() {
                panic!("size of `cmsgcred_packed` did not match that of `cmsgcred`");
            }
        };
        unsafe {
            // SAFETY: the two types have the same layout, save for stricter padding of the input
            &*<*const _>::cast(self)
        }
    }
}
