use std::fmt::{self, Debug, Formatter};

impmod! {
    local_socket::peer_creds,
    PeerCreds as Inner,
    Pid,
}

/// Credentials of a connection peer.
///
/// The design of this type is inspired by [`std::fs::Metadata`].
///
/// The nature of the credentials is OS-specific and using some of them for making security
/// decisions may be subject to race conditions. Please make sure that the identifiers you use for
/// authenticating the peer cannot be invalidated and reused without administrative intervention.
///
/// ## Platform-specific behavior
/// Platforms can be divided by the set of credentials provided in this struct into the following
/// categories:
/// - Windows
/// - `ucred`-based
///   - Linux (including Android)
///   - OpenBSD
///   - Fuchsia
///   - Redox
/// - `xucred`-based
///   - FreeBSD
///   - DragonFly BSD
///   - Darwin (macOS, iOS, tvOS, watchOS)
/// - NetBSD
///
/// See the getter-level documentation for which credentials are available on which categories.
#[derive(Copy, Clone)]
pub struct PeerCreds(Inner);
impl Debug for PeerCreds {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { Debug::fmt(&self.0, f) }
}
impl From<Inner> for PeerCreds {
    #[inline(always)]
    fn from(v: Inner) -> Self { Self(v) }
}
impl PeerCreds {
    /// Returns the process ID of the peer.
    ///
    /// Please note that looking up security identifiers and other authentication data by process
    /// ID is subject to race conditions, assuming the peer process exiting and its PID being
    /// reused does not terminate the connection. This may happen if the peer's handle/FD for the
    /// connection gets leaked by mistake (perhaps by being inherited by a child process or sent
    /// via `SCM_RIGHTS`). It is not possible to cause this race in Interprocess, but
    /// adverse interactions with other libraries may require mitigating the race by performing
    /// multiple lookups.
    ///
    /// # Platform-specific behavior
    /// Available on:
    /// - Windows
    /// - `ucred`-based platforms
    /// - FreeBSD 12.3 and newer
    #[inline]
    pub fn pid(&self) -> Option<Pid> { self.0.pid() }

    /// Returns the effective user ID of the peer.
    ///
    /// # Platform-specific behavior
    /// Available on:
    /// - `ucred`-based platforms
    /// - `xucred`-based platforms
    /// - NetBSD
    #[cfg(any(doc, unix))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
    #[inline]
    pub fn euid(&self) -> Option<uid_t> { self.0.euid() }

    /// Returns the effective group ID of the peer.
    ///
    /// # Platform-specific behavior
    /// Available on:
    /// - `ucred`-based platforms
    /// - `xucred`-based platforms
    /// - NetBSD
    #[cfg(any(doc, unix))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
    #[inline]
    pub fn egid(&self) -> Option<gid_t> { self.0.egid() }

    /// Returns a slice containing the set of supplementary group IDs of the peer, truncated to a
    /// platform-specific limit. Use [`groups_truncated`](Self::groups_truncated) to determine if
    /// the information may be incomplete.
    ///
    /// `None` will be returned on platforms where only the effective group ID of the process
    /// is available, emphasizing the need for fallback behavior on the caller's end.
    ///
    /// It is unspecified whether this contains the effective group ID of the peer or not. Make
    /// sure to get the [`egid`](Self::egid) to include or exclude it from prvilege checks
    /// depending on your needs.
    ///
    /// # Platform-specific behavior
    /// Available on:
    /// - `xucred`-based platforms
    #[cfg(any(doc, unix))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
    #[inline]
    pub fn groups(&self) -> Option<&[gid_t]> { self.0.groups() }

    /// Returns true if it may be the case that the return value of [`groups`](Self::groups) is
    /// not the complete set of supplementary group IDs of the peer.
    ///
    /// Returns false if `groups` returns `None`.
    #[cfg(any(doc, unix))]
    #[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
    #[inline]
    pub fn groups_truncated(&self) -> bool { self.0.groups_truncated() }
}

#[cfg(unix)]
use libc::{gid_t, uid_t};
#[cfg(not(unix))]
#[cfg_attr(not(doc), allow(unused_imports))]
use {u32 as uid_t, u32 as gid_t};
