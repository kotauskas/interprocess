use super::*;

#[cfg(any(
    target_os = "android",
    target_os = "fuchsia",
    target_os = "linux",
    target_os = "redox",
))]
pub type Inner = libc::ucred;
#[cfg(target_os = "openbsd")]
pub type Inner = libc::sockpeercred;

#[cfg_attr(test, allow(clippy::panic_in_result_fn))]
pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    // SAFETY: ucred/sockpeercred is the correct struct for SO_PEERCRED
    let cr = unsafe { c_wrappers::getsockopt::<Inner>(fd, libc::SOL_SOCKET, libc::SO_PEERCRED)? };

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

    #[cfg(all(test, target_os = "openbsd"))]
    {
        let eids = super::peereid::peer_creds(fd).unwrap();
        assert_eq!((cr.uid, cr.gid), eids);
    }

    Ok(cr)
}

pub fn euid(cr: &Inner) -> Option<uid_t> { Some(cr.uid) }
pub fn egid(cr: &Inner) -> Option<gid_t> { Some(cr.gid) }
pub fn groups(_: &Inner) -> Option<&[gid_t]> { None }
pub fn groups_truncated(_: &Inner) -> bool { false }
pub fn pid(cr: &Inner) -> Option<pid_t> { Some(cr.pid) }

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("PeerCreds")
        .field("euid", &euid(cr))
        .field("egid", &egid(cr))
        .field("pid", &pid(cr))
        .finish()
}
