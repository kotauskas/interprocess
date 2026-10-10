use {
    crate::os::unix::{c_wrappers, unixprelude::*},
    std::{
        io,
        mem::{size_of, MaybeUninit},
    },
};

pub type Pid = pid_t;

#[derive(Copy, Clone, Debug)]
pub struct PeerCreds(Inner);
impl PeerCreds {
    pub(crate) fn for_socket(fd: BorrowedFd<'_>) -> io::Result<Self> {
        let mut inner = MaybeUninit::<Inner>::uninit();
        c_wrappers::getsockopt_raw(
            fd,
            CRED_OPTLEVEL,
            CRED_OPTNAME,
            &mut inner,
            size_of::<Inner>(),
        )?;

        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        ))]
        {
            const CR_VERSION_OFFSET: usize = offset_of!(libc::xucred, cr_version);
            let vers = unsafe {
                inner.as_ptr().byte_add(CR_VERSION_OFFSET).cast::<libc::c_uint>().read()
            };
            if vers != libc::XUCRED_VERSION {
                crate::misc::cold_path();
                // The manpage tells us to check the value but doesn't say what we are to do in
                // the case of a mismatch. Per the implementation of getpeereid, the """correct"""
                // way of handling this error is to shrivel up and cry if cr_version is not equal
                // to XUCRED_VERSION. This is also done in basically every web search result for
                // XUCRED_VERSION, including PostgreSQL, mdnsresponder, and the FreeBSD port of
                // the Wayland libraries.
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "xucred cr_version mismatch",
                ));
            }
        }

        // SAFETY: the getsockopt operation initializes every field, and we've checked for a very
        // hypothetical version mismatch on xucred platforms
        let inner = unsafe { inner.assume_init() };

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
            target_os = "fuchsia",
        ))]
        if inner.pid == 0 {
            // Yes, a Linux kernel developer really thought that zero-initializing a struct that
            // contains a UID field was a good mechanism for representing an obscure sentinel
            return Err(io::Error::from(io::ErrorKind::ConnectionReset));
        }
        Ok(Self(inner))
    }

    pub fn pid(&self) -> Option<pid_t> {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
            target_os = "fuchsia",
            target_os = "openbsd",
        ))]
        return Some(self.0.pid);
        #[cfg(target_os = "freebsd")]
        {
            // SAFETY:
            // In sys/kern/uipc_socket.c, the function soalloc, which creates Unix domain sockets,
            // allocates a struct socket* using uma_zalloc with the M_ZERO flag, which returns
            // a bzeroed allocation whose fields are then modified to set up a Unix domain
            // socket, which includes casting the struct socket* to a struct unpcb* to fill out
            // its various fields, including unp_peercred, which is a struct xucred (with the
            // same fields and layout as its userspace incarnation) that the implementation of
            // getsockopt(LOCAL_PEERCRED) in sys/kern/uipc_usrreq.c then copies into userspace
            // using sooptcopyout. Before FreeBSD 12.3, that struct xucred's union of a reserved
            // void* field and cr_pid is just that reserved field, and pid_t is smaller than
            // void*, meaning that in FreeBSD kernels before 12.3, the space that goes on to be
            // used for cr_pid in FreeBSD 12.3 is zero-initialized once on socket creation and
            // then never modified, meaning that the below union field access is always an access
            // to well-initialized memory, even on FreeBSD versions whose headers do not include a
            // cr_pid field.
            let val = unsafe { self.0.cr_pid__c_anonymous_union.cr_pid };
            return (val != 0).then_some(val);
        }
        #[cfg(target_os = "netbsd")]
        return Some(self.0.unp_pid);
        #[allow(unreachable_code)]
        None
    }
    pub fn euid(&self) -> Option<uid_t> {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
            target_os = "fuchsia",
            target_os = "openbsd",
        ))]
        return Some(self.0.uid);
        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        ))]
        return Some(self.0.cr_uid);
        #[cfg(target_os = "netbsd")]
        return Some(self.0.unp_euid);
        #[allow(unreachable_code)]
        None
    }
    pub fn egid(&self) -> Option<uid_t> {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
            target_os = "fuchsia",
            target_os = "openbsd",
        ))]
        return Some(self.0.gid);
        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        ))]
        // FreeBSD sys/sys/ucred.h:114
        return self.groups_raw().first().copied();
        #[cfg(target_os = "netbsd")]
        return Some(self.0.unp_egid);
        #[allow(unreachable_code)]
        None
    }
    pub fn groups(&self) -> Option<&[gid_t]> {
        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        ))]
        #[allow(clippy::indexing_slicing)]
        // The first value is the EGID, so ignore it to match the output of getgroups.
        return Some(&self.groups_raw()[1..]);
        #[allow(unreachable_code)]
        None
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
    ))]
    // validity of cr_ngroups promised by kernel
    #[allow(clippy::indexing_slicing, clippy::cast_sign_loss)]
    fn groups_raw(&self) -> &[gid_t] { &self.0.cr_groups[..self.0.cr_ngroups as usize] }

    #[allow(clippy::cast_sign_loss)] // validity of cr_ngroups promised by kernel
    pub fn groups_truncated(&self) -> bool {
        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        ))]
        return (self.0.cr_ngroups as usize) >= self.0.cr_groups.len();
        #[allow(unreachable_code)]
        false
    }
}

#[cfg(target_os = "openbsd")]
use libc::{sockpeercred as Inner, SOL_SOCKET as CRED_OPTLEVEL, SO_PEERCRED as CRED_OPTNAME};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "redox",
    target_os = "fuchsia",
))]
use libc::{ucred as Inner, SOL_SOCKET as CRED_OPTLEVEL, SO_PEERCRED as CRED_OPTNAME};
#[cfg(target_os = "netbsd")]
use {
    libc::{unpcbid as Inner, LOCAL_PEEREID as CRED_OPTNAME},
    SOL_LOCAL_LIBC_CRATE_DOESNT_HAVE_IT as CRED_OPTLEVEL,
};
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
))]
use {
    libc::{xucred as Inner, LOCAL_PEERCRED as CRED_OPTNAME},
    SOL_LOCAL_LIBC_CRATE_DOESNT_HAVE_IT as CRED_OPTLEVEL,
};

#[allow(unused)]
const SOL_LOCAL_LIBC_CRATE_DOESNT_HAVE_IT: c_int = 0;
