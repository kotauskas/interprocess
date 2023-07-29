#[cfg(uds_sockcred2)]
use libc::sockcred2;
#[cfg(uds_xucred)]
use libc::xucred;
use libc::{c_int, c_short, cmsgcred, gid_t, pid_t, uid_t};
use std::{cmp::min, marker::PhantomData, mem::size_of, ptr::addr_of};
use to_method::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Credentials<'a> {
    Cmsgcred(&'a cmsgcred_packed),
    #[cfg(uds_sockcred2)]
    Sockcred2(&'a sockcred2_packed),
    #[cfg(uds_xucred)]
    Xucred(xucred),
}
impl<'a> Credentials<'a> {
    pub fn euid(&self) -> Option<uid_t> {
        match self {
            Self::Cmsgcred(c) => Some(c.cmcred_euid),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => Some(c.sc_euid),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => Some(c.cr_uid),
        }
    }
    pub fn ruid(&self) -> Option<uid_t> {
        match self {
            Self::Cmsgcred(c) => Some(c.cmcred_uid),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => Some(c.sc_uid),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => None,
        }
    }
    pub fn egid(&self) -> Option<gid_t> {
        match self {
            Self::Cmsgcred(..) => None,
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => Some(c.sc_egid),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => None,
        }
    }
    pub fn rgid(&self) -> Option<gid_t> {
        match self {
            Self::Cmsgcred(c) => Some(c.cmcred_gid),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => Some(c.sc_gid),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => None,
        }
    }
    pub fn pid(&self) -> Option<pid_t> {
        match self {
            Self::Cmsgcred(c) => Some(c.cmcred_pid),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => Some(c.sc_pid),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => None, // TODO available on FreeBSD, but extremely scuffed
        }
    }
    fn n_groups(&self) -> usize {
        match self {
            Self::Cmsgcred(c) => min(c.cmcred_ngroups, libc::CMGROUP_MAX as _).to::<c_int>(),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => c.sc_ngroups,
            #[cfg(uds_xucred)]
            Self::Xucred(c) => c.cr_ngroups,
        }
        .try_to::<usize>()
        .unwrap()
    }
    fn ptr_to_gids(&self) -> *const gid_packed {
        match self {
            Self::Cmsgcred(c) => addr_of!(c.cmcred_groups).cast::<gid_packed>(),
            #[cfg(uds_sockcred2)]
            Self::Sockcred2(c) => addr_of!(c.sc_groups).cast::<gid_packed>(),
            #[cfg(uds_xucred)]
            Self::Xucred(c) => addr_of!(c.cr_groups).cast::<gid_packed>(),
        }
    }
    pub fn groups(&self) -> Groups<'a> {
        let cur = self.ptr_to_gids();
        let end = unsafe {
            // SAFETY: this puts us one byte past the last one
            cur.add(self.n_groups())
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
    // Breaks on batshit insane platforms where c_short = c_int,
    // but neither FreeBSD nor Dragonfly BSD belong to this category
    pub __pad0: c_short,
    pub cmcred_groups: [gid_t; libc::CMGROUP_MAX],
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

#[cfg(uds_sockcred2)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub(crate) struct sockcred2_packed {
    pub sc_version: c_int,
    pub sc_pid: pid_t,
    pub sc_uid: uid_t,
    pub sc_euid: uid_t,
    pub sc_gid: gid_t,
    pub sc_egid: gid_t,
    pub sc_ngroups: c_int,
    pub sc_groups: [gid_t; 1],
}
impl AsRef<sockcred2_packed> for sockcred2 {
    fn as_ref(&self) -> &sockcred2_packed {
        const _: () = {
            if size_of::<sockcred2_packed>() != size_of::<sockcred2>() {
                panic!("size of `sockcred2_packed` did not match that of `sockcred2`");
            }
        };
        unsafe {
            // SAFETY: the two types have the same layout, save for stricter padding of the input
            &*<*const _>::cast(self)
        }
    }
}
