use {super::*, crate::os::unix::c_wrappers};

#[cfg(any(
    target_os = "android",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "linux",
    target_os = "redox",
))]
pub type Inner = libc::ucred;
#[cfg(target_os = "netbsd")]
pub type Inner = libc::unpcbid;
#[cfg(target_os = "openbsd")]
pub type Inner = libc::sockpeercred;

#[cfg(not(target_os = "netbsd"))]
const LEVEL_AND_OPT: (c_int, c_int) = (libc::SOL_SOCKET, libc::SO_PEERCRED);
#[cfg(target_os = "netbsd")]
const LEVEL_AND_OPT: (c_int, c_int) = (0, libc::LOCAL_PEEREID); // SOL_LOCAL

#[cfg_attr(test, allow(clippy::missing_assert_message, clippy::panic_in_result_fn))]
pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    // SAFETY: ucred/unpcbid/sockpeercred is the correct struct for SO_PEERCRED
    let (level, opt) = LEVEL_AND_OPT;
    let cr = unsafe { c_wrappers::getsockopt::<Inner>(fd, level, opt)? };

    #[cfg(any(
        target_os = "android",
        target_os = "fuchsia",
        target_os = "linux",
        target_os = "redox",
    ))]
    if cr.pid == 0 {
        // Yes, a Linux kernel developer really thought that zero-initializing a struct that
        // contains a UID field was a good mechanism for representing an obscure sentinel
        return Err(io::Error::from(io::ErrorKind::ConnectionReset));
    }

    #[cfg(all(test, any(target_os = "netbsd", target_os = "openbsd")))]
    {
        let eids = super::peereid::peer_creds(fd).unwrap();
        assert_eq!((euid(&cr).unwrap(), egid(&cr).unwrap()), eids);
    }

    Ok(cr)
}

#[cfg(not(target_os = "netbsd"))]
mod getters {
    use super::*;
    pub fn euid(cr: &Inner) -> Option<uid_t> { Some(cr.uid) }
    pub fn egid(cr: &Inner) -> Option<gid_t> { Some(cr.gid) }
    pub fn pid(cr: &Inner) -> Option<pid_t> { Some(cr.pid) }
}
#[cfg(target_os = "netbsd")]
mod getters {
    use super::*;
    pub fn euid(cr: &Inner) -> Option<uid_t> { Some(cr.unp_euid) }
    pub fn egid(cr: &Inner) -> Option<gid_t> { Some(cr.unp_egid) }
    pub fn pid(cr: &Inner) -> Option<pid_t> { Some(cr.unp_pid) }
}
pub use getters::*;
pub fn groups(_: &Inner) -> Option<&[gid_t]> { None }
pub fn groups_truncated(_: &Inner) -> bool { false }

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("PeerCreds")
        .field("euid", &euid(cr))
        .field("egid", &egid(cr))
        .field("pid", &pid(cr))
        .finish()
}
