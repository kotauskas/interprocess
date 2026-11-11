use {super::*, std::mem::MaybeUninit};

pub type Inner = libc::xucred;

const SOL_LOCAL: c_int = 0;

#[cfg_attr(test, allow(clippy::panic_in_result_fn))]
pub fn peer_creds(fd: BorrowedFd<'_>) -> io::Result<Inner> {
    let mut inner = MaybeUninit::<Inner>::uninit();
    c_wrappers::getsockopt_raw(
        fd,
        SOL_LOCAL,
        libc::LOCAL_PEERCRED,
        &mut inner,
        size_of::<Inner>(),
    )?;

    const CR_VERSION_OFFSET: usize = offset_of!(libc::xucred, cr_version);
    let vers =
        unsafe { inner.as_ptr().byte_add(CR_VERSION_OFFSET).cast::<libc::c_uint>().read() };
    if vers != libc::XUCRED_VERSION {
        crate::misc::cold_path();
        // The manpage tells us to check the value but doesn't say what we are to do in the case
        // of a mismatch. Per the implementation of getpeereid, the """correct""" way of handling
        // this error is to shrivel up and cry if cr_version is not equal to XUCRED_VERSION.
        // This is also done in basically every web search result for XUCRED_VERSION, including
        // PostgreSQL, mdnsresponder, and the FreeBSD port of the Wayland libraries.
        return Err(io::Error::new(io::ErrorKind::InvalidData, "xucred cr_version mismatch"));
    }

    // SAFETY: the getsockopt operation initializes every field, and we've checked for a very
    // hypothetical version mismatch
    let inner = unsafe { inner.assume_init() };

    #[cfg(test)]
    {
        let eids = super::peereid::peer_creds(fd).unwrap();
        assert_eq!((inner.cr_uid, inner.cr_groups[0]), eids);
    }

    Ok(inner)
}

pub fn euid(cr: &Inner) -> Option<uid_t> { Some(cr.cr_uid) }
// FreeBSD sys/sys/ucred.h:114
pub fn egid(cr: &Inner) -> Option<gid_t> { groups_raw(cr).first().copied() }

#[allow(clippy::indexing_slicing)]
pub fn groups(cr: &Inner) -> Option<&[gid_t]> {
    // The first value is the EGID, so ignore it to match the output of getgroups.
    Some(&groups_raw(cr)[1..])
}
// validity of cr_ngroups promised by kernel
#[allow(clippy::indexing_slicing, clippy::cast_sign_loss)]
fn groups_raw(cr: &Inner) -> &[gid_t] { &cr.cr_groups[..cr.cr_ngroups as usize] }
#[allow(clippy::cast_sign_loss)] // validity of cr_ngroups promised by kernel
pub fn groups_truncated(cr: &Inner) -> bool { (cr.cr_ngroups as usize) >= cr.cr_groups.len() }

#[cfg(target_os = "freebsd")]
pub fn pid(cr: &Inner) -> Option<pid_t> {
    #[cfg(target_os = "freebsd")]
    {
        // SAFETY:
        // In sys/kern/uipc_socket.c, the function soalloc, which creates Unix domain sockets,
        // allocates a struct socket* using uma_zalloc with the M_ZERO flag, which returns a
        // bzeroed allocation whose fields are then modified to set up a Unix domain socket, which
        // includes casting the struct socket* to a struct unpcb* to fill out its various fields,
        // including unp_peercred, which is a struct xucred (with the same fields and layout as
        // its userspace incarnation) that the implementation of getsockopt(LOCAL_PEERCRED) in
        // sys/kern/uipc_usrreq.c then copies into userspace using sooptcopyout. Before FreeBSD
        // 12.3, that struct xucred's union of a reserved void* field and cr_pid is just that
        // reserved field, and pid_t is smaller than void*, meaning that in FreeBSD kernels before
        // 12.3, the space that goes on to be used for cr_pid in FreeBSD 12.3 is zero-initialized
        // once on socket creation and then never modified, meaning that the below union field
        // access is always an access to well-initialized memory, even on FreeBSD versions whose
        // headers do not include a cr_pid field.
        let val = unsafe { cr.cr_pid__c_anonymous_union.cr_pid };
        (val != 0).then_some(val)
    }
    #[cfg(not(target_os = "freebsd"))]
    {
        None
    }
}

pub fn fmt(cr: &Inner, f: &mut Formatter<'_>) -> fmt::Result {
    let mut ds = f.debug_struct("PeerCreds");
    ds.field("euid", &euid(cr)).field("egid", &egid(cr));
    if cfg!(target_os = "freebsd") {
        ds.field("pid", &pid(cr));
    }
    ds.finish()
}
