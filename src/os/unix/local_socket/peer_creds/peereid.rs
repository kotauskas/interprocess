use super::*;

pub type Inner = (uid_t, gid_t);

pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    // Zero-initializing UIDs and GIDs would be asking for trouble
    let (mut euid, mut egid) = (!0, !0);
    if unsafe { libc::getpeereid(fd.as_raw_fd(), &raw mut euid, &raw mut egid) == -1 } {
        return Err(io::Error::last_os_error());
    }
    Ok((euid, egid))
}

pub fn euid(&(euid, _): &Inner) -> Option<uid_t> { Some(euid) }
pub fn egid(&(_, egid): &Inner) -> Option<gid_t> { Some(egid) }
pub fn groups(_: &Inner) -> Option<&[gid_t]> { None }
pub fn groups_truncated(_: &Inner) -> bool { false }
pub fn pid(_: &Inner) -> Option<pid_t> { None }

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("PeerCreds").field("euid", &euid(cr)).field("egid", &egid(cr)).finish()
}
