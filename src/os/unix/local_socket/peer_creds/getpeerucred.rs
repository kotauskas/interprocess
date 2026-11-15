use {super::*, crate::OrErrno};

// getpeerucred hands back a heap-allocated ucred_t that must be freed with ucred_free. PeerCreds
// is required to be Copy, so this is a copy of the requisite fields before it is freed.
#[derive(Copy, Clone)]
pub struct Inner {
    pid: Option<pid_t>,
    euid: Option<uid_t>,
    egid: Option<gid_t>,
    groups: Option<Groups>,
}

// 16 is illumos's default ngroups_max, so it is sufficient for the full list of groups unless
// that limit is raised by the system administrator. (If there really are more than 16 groups,
// this fact is reported by groups_truncated.)
const MAX_GROUPS: usize = 16;

#[derive(Copy, Clone)]
struct Groups {
    list: [gid_t; MAX_GROUPS],
    // Invariant: len <= MAX_GROUPS, upheld by Groups::copy_from.
    len: usize,
    truncated: bool,
}

impl Groups {
    fn copy_from(groups: &[gid_t]) -> Self {
        let mut list = [0; MAX_GROUPS];
        for (dst, src) in list.iter_mut().zip(groups) {
            *dst = *src;
        }
        Self { list, len: groups.len().min(MAX_GROUPS), truncated: groups.len() > MAX_GROUPS }
    }

    fn as_slice(&self) -> &[gid_t] {
        self.list.get(..self.len).expect("copy_from capped len at MAX_GROUPS")
    }
}

struct UcredGuard(*mut libc::ucred_t);

impl Drop for UcredGuard {
    fn drop(&mut self) {
        // SAFETY: a UcredGuard is built from a pointer that getpeerucred allocated, and is only
        // freed here.
        unsafe { libc::ucred_free(self.0) }
    }
}

pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    let mut ucred: *mut libc::ucred_t = std::ptr::null_mut();
    // SAFETY: fd is a live borrowed fd, and a null *ucred makes getpeerucred allocate a fresh
    // ucred_t. (Can we reuse a ucred_t? This would require state outside this function.)
    unsafe { libc::getpeerucred(fd.as_raw_fd(), &mut ucred) != -1 }.true_val_or_errno(())?;
    let ucred = UcredGuard(ucred);

    // SAFETY: ucred.0 is a valid ucred_t while the UcredGuard is alive.
    let (pid, euid, egid) = unsafe {
        // The ucred_get* functions return -1 (so !0 for uid_t/gid_t) when the field is absent.
        (libc::ucred_getpid(ucred.0), libc::ucred_geteuid(ucred.0), libc::ucred_getegid(ucred.0))
    };
    Ok(Inner {
        pid: (pid != -1).then_some(pid),
        euid: (euid != !0).then_some(euid),
        egid: (egid != !0).then_some(egid),
        groups: read_groups(&ucred),
    })
}

fn read_groups(ucred: &UcredGuard) -> Option<Groups> {
    let mut list: *const gid_t = std::ptr::null();
    // SAFETY: ucred.0 is a valid ucred_t while the UcredGuard is alive (which it is guaranteed to
    // be because we have a shared reference to it). list is a writable *const gid_t.
    let count = unsafe { libc::ucred_getgroups(ucred.0, &mut list) };
    // Like the other ucred_get* functions (see peer_creds above), -1 means the ucred_t has no
    // group information.
    let count = usize::try_from(count).ok()?;
    if count == 0 {
        // When count is 0, the list may be null or dangling. That isn't allowed by from_raw_parts
        // so we make our own list instead.
        return Some(Groups::copy_from(&[]));
    }
    // SAFETY: list points at count gid_t instances inside the ucred_t.
    let groups = unsafe { std::slice::from_raw_parts(list, count) };
    Some(Groups::copy_from(groups))
}

pub fn euid(cr: &Inner) -> Option<uid_t> { cr.euid }
pub fn egid(cr: &Inner) -> Option<gid_t> { cr.egid }
pub fn groups(cr: &Inner) -> Option<&[gid_t]> { cr.groups.as_ref().map(Groups::as_slice) }
pub fn groups_truncated(cr: &Inner) -> bool { cr.groups.is_some_and(|groups| groups.truncated) }
pub fn pid(cr: &Inner) -> Option<pid_t> { cr.pid }

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("PeerCreds")
        .field("euid", &euid(cr))
        .field("egid", &egid(cr))
        .field("pid", &pid(cr))
        .field("groups", &groups(cr))
        .field("groups_truncated", &groups_truncated(cr))
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_from_truncates_past_max_groups() {
        let all: Vec<gid_t> = (0..=MAX_GROUPS)
            .map(|i| gid_t::try_from(i).expect("test group count fits in gid_t"))
            .collect();
        let (exactly_max, _) = all.split_at(MAX_GROUPS);

        let groups = Groups::copy_from(exactly_max);
        assert_eq!(groups.as_slice(), exactly_max);
        assert!(!groups.truncated);

        let groups = Groups::copy_from(&all);
        assert_eq!(groups.as_slice(), exactly_max);
        assert!(groups.truncated);
    }
}
