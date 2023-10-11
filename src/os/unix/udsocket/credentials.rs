//! Credential table for portable secure authentication.
//!
//! [`Credentials`] is the table type itself – see its own documentation for more on where and how it is used.
//! [`Groups`] is an iterator produced by `Credentials` that enumerates supplementary groups stored in the table.

use crate::os::unix::unixprelude::*;
#[cfg(uds_cmsgcred)]
use libc::cmsgcred;
#[cfg(uds_sockcred2)]
use libc::sockcred2;
#[cfg(uds_ucred)]
use libc::ucred;
#[cfg(uds_xucred)]
use libc::xucred;
use std::{iter::FusedIterator, marker::PhantomData, mem::size_of};
#[allow(unused_imports)]
use {
    std::{cmp::min, ptr::addr_of},
    to_method::To,
};

/// A table of credentials for portable secure authentication.
///
/// # Dedicated peer credentials querying
/// TODO talk here about peercred in `UdSocket`
///
/// # Ancillary message
///
/// This struct actually doubles as an ancillary data message that allows receiving the credentials of the peer process
/// and, on some systems, setting the contents of this ancillary message that the other process will receive.
///
/// To receive this message, an appropriate socket option must be enabled. There are actually two main types of such
/// socket options: one-shot credentials reception and continuous ("persistent" in FreeBSD parlance) credentials spam.
/// The latter option is primarily useful with datagram sockets, which are connectionless.
/// After one of those those types of options is enabled, either every receive operation that provides an ancillary data
/// buffer, or just the next one, will receive an instance of this message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Credentials<'a>(pub(super) CredentialsInner<'a>);
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum CredentialsInner<'a> {
    #[cfg(uds_ucred)]
    AncUcred(&'a ucred_packed),
    #[cfg(uds_ucred)]
    Ucred(ucred),
    #[cfg(uds_cmsgcred)]
    Cmsgcred(&'a cmsgcred_packed),
    #[cfg(uds_sockcred2)]
    Sockcred2(&'a sockcred2_packed),
    #[cfg(uds_xucred)]
    Xucred(xucred, PhantomData<&'a xucred>),
}
/// Methods that read the received/stored credentials.
impl<'a> Credentials<'a> {
    /// Returns the **effective** user ID stored in the credentials table, or `None` if no such information is
    /// available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return the UID from `ucred` despite the Linux kernel allowing either the effective or the real UID
    /// to be sent.
    #[inline]
    pub fn euid(&self) -> Option<uid_t> {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(c) => Some(c.uid),
            #[cfg(uds_ucred)]
            CredentialsInner::Ucred(c) => Some(c.uid),
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => Some(c.cmcred_euid),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => Some(c.sc_euid),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(c, _) => Some(c.cr_uid),
        }
    }
    /// Returns the **real** user ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return `None`, even though `ucred` may contain either the effective or the real UID; this is because
    /// there is no way of detecting which of those two the other process sent.
    #[inline]
    pub fn ruid(&self) -> Option<uid_t> {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(..) | CredentialsInner::Ucred(..) => None,
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => Some(c.cmcred_uid),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => Some(c.sc_uid),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(..) => None,
        }
    }
    /// Returns the **closest thing to the real user ID** among what's stored in the credentials table. If a real UID is
    /// not present, the effective UID is returned instead.
    ///
    /// This method is intended to be used by daemons which need to verify user input for security purposes and would
    /// like to see past elevation via `setuid` programs if possible.
    pub fn best_effort_ruid(&self) -> uid_t {
        match (self.euid(), self.ruid()) {
            (Some(id), ..) | (None, Some(id)) => id,
            (None, None) => unreachable!(),
        }
    }
    /// Returns the **effective** group ID stored in the credentials table, or `None` if no such information is
    /// available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return the GID from `ucred` despite the Linux kernel allowing either the effective or the real GID
    /// to be sent.
    #[inline]
    pub fn egid(&self) -> Option<gid_t> {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(c) => Some(c.gid),
            #[cfg(uds_ucred)]
            CredentialsInner::Ucred(c) => Some(c.gid),
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(..) => None,
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => Some(c.sc_egid),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(..) => None,
        }
    }
    /// Returns the **real** group ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Will always return `None`, even though `ucred` may contain either the effective or the real GID; this is because
    /// there is no way of detecting which of those two the other process sent.
    #[inline]
    pub fn rgid(&self) -> Option<gid_t> {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(..) | CredentialsInner::Ucred(..) => None,
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => Some(c.cmcred_gid),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => Some(c.sc_gid),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(..) => None,
        }
    }
    /// Returns the **closest thing to the real group ID** among what's stored in the credentials table. If a real GID
    /// is not present, the effective GID is returned instead.
    ///
    /// This method is intended to be used by daemons which need to verify user input for security purposes and would
    /// like to see past elevation via `setuid` programs if possible.
    pub fn best_effort_rgid(&self) -> gid_t {
        match (self.egid(), self.rgid()) {
            (Some(id), ..) | (None, Some(id)) => id,
            (None, None) => unreachable!(),
        }
    }
    /// Returns the process ID stored in the credentials table, or `None` if no such information is available.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Privileged processes (those with `CAP_SYS_ADMIN`) may send any PID, as long as it belongs to an existing
    /// process. Note that actually relying on the kernel's check for PID validity is a possible [TOCTOU] bug.
    ///
    /// [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use
    #[inline]
    pub fn pid(&self) -> Option<pid_t> {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(c) => Some(c.pid),
            #[cfg(uds_ucred)]
            CredentialsInner::Ucred(c) => Some(c.pid),
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => Some(c.cmcred_pid),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => Some(c.sc_pid),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(..) => None, // TODO available on FreeBSD, but extremely scuffed
        }
    }
    /// Returns an iterator over the supplementary groups in the credentials table.
    ///
    /// The resulting iterator implements `ExactSizeIterator`, so the amount of supplementary groups can be queried
    /// without iterating through all via the `.len()` method.
    ///
    /// # Platform-specific behavior
    /// ## `ucred` (Linux)
    /// Always empty.
    #[inline]
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

    fn n_groups(&self) -> usize {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(..) | CredentialsInner::Ucred(..) => 0_usize,
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => min(c.cmcred_ngroups, libc::CMGROUP_MAX as _).to::<c_int>(),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => c.sc_ngroups,
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(c, _) => c.cr_ngroups.to::<c_int>(),
        }
        .try_to::<usize>()
        .unwrap()
    }
    fn ptr_to_gids(&self) -> *const gid_packed {
        match self.0 {
            #[cfg(uds_ucred)]
            CredentialsInner::AncUcred(..) | CredentialsInner::Ucred(..) => std::ptr::null(),
            #[cfg(uds_cmsgcred)]
            CredentialsInner::Cmsgcred(c) => addr_of!(c.cmcred_groups).cast::<gid_packed>(),
            #[cfg(uds_sockcred2)]
            CredentialsInner::Sockcred2(c) => addr_of!(c.sc_groups).cast::<gid_packed>(),
            #[cfg(uds_xucred)]
            CredentialsInner::Xucred(c, _) => addr_of!(c.cr_groups).cast::<gid_packed>(),
        }
    }
}

/// An iterator over supplementary groups stored in [`Credentials`].
///
/// # Platform-specific behavior
/// ## `ucred` (Linux)
/// Always empty.
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
impl FusedIterator for Groups<'_> {}

#[allow(non_camel_case_types)]
type gid_packed = [u8; size_of::<gid_t>()];

#[cfg(uds_ucred)]
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub(crate) struct ucred_packed {
    pid: pid_t,
    uid: uid_t,
    gid: gid_t,
}
#[cfg(uds_ucred)]
impl AsRef<ucred_packed> for libc::ucred {
    fn as_ref(&self) -> &ucred_packed {
        const _: () = {
            if size_of::<ucred_packed>() != size_of::<libc::ucred>() {
                panic!("size of `ucred_packed` did not match that of `ucred`");
            }
        };
        unsafe {
            // SAFETY: the two types have the same layout, save for stricter padding of the input
            &*<*const _>::cast(self)
        }
    }
}

#[cfg(uds_cmsgcred)]
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
#[cfg(uds_cmsgcred)]
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
#[cfg(uds_sockcred2)]
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
