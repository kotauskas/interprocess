use {
    crate::os::unix::{c_wrappers, unixprelude::*},
    std::{
        fmt::{self, Debug, Formatter},
        io,
    },
};

pub type Pid = pid_t;

macro_rules! mod_imp {
    ($(#[cfg($($cfg:tt)+)] mod $mod:ident);+ $(;)?) => {$(
        #[cfg($($cfg)+)]
        mod $mod;
        #[cfg($($cfg)+)]
        use $mod::*;
    )+};
}

mod_imp! {
    #[cfg(any(
        target_os = "android",
        target_os = "fuchsia",
        target_os = "linux",
        target_os = "openbsd",
        target_os = "redox",
    ))]
    mod ucred_like;
    #[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_vendor = "apple"))]
    mod xucred;
    #[cfg(any(target_os = "nto", target_os = "qnx"))]
    mod peereid;
    #[cfg(target_os = "netbsd")]
    mod unpcbid;
}

// If we don't pull in this module on the BSDs, nothing is going to check if it compiles
#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_vendor = "apple"
))]
#[allow(unused)]
mod peereid;

#[derive(Copy, Clone)]
pub struct PeerCreds(Inner);
impl PeerCreds {
    pub fn for_socket(fd: BorrowedFd<'_>) -> io::Result<Self> { peer_creds(fd).map(Self) }
    pub fn pid(&self) -> Option<pid_t> { pid(&self.0) }
    pub fn euid(&self) -> Option<uid_t> { euid(&self.0) }
    pub fn egid(&self) -> Option<uid_t> { egid(&self.0) }
    pub fn groups(&self) -> Option<&[gid_t]> { groups(&self.0) }
    pub fn groups_truncated(&self) -> bool { groups_truncated(&self.0) }
}
impl Debug for PeerCreds {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { fmt(&self.0, f) }
}
