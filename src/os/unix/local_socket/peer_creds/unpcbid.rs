use super::*;

pub type Inner = libc::unpcbid;

const SOL_LOCAL: c_int = 0;

#[cfg_attr(test, allow(clippy::panic_in_result_fn))]
pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    // SAFETY: unpcbid is the correct struct for LOCAL_PEEREID
    let cr = unsafe { c_wrappers::getsockopt::<Inner>(fd, SOL_LOCAL, libc::LOCAL_PEEREID)? };

    #[cfg(test)]
    {
        let eids = super::peereid::peer_creds(fd).unwrap();
        assert_eq!((cr.unp_euid, cr.unp_egid), eids);
    }

    Ok(cr)
}
pub fn euid(cr: &Inner) -> Option<uid_t> { Some(cr.unp_euid) }
pub fn egid(cr: &Inner) -> Option<gid_t> { Some(cr.unp_egid) }
pub fn groups(_: &Inner) -> Option<&[gid_t]> { None }
pub fn groups_truncated(_: &Inner) -> bool { false }
pub fn pid(cr: &Inner) -> Option<pid_t> { Some(cr.unp_pid) }

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("PeerCreds")
        .field("euid", &euid(cr))
        .field("egid", &egid(cr))
        .field("pid", &pid(cr))
        .finish()
}
