#[allow(unused_imports)]
use crate::{FdOrErrno, OrErrno};
#[cfg(target_os = "android")]
use std::os::android::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
use {
    super::unixprelude::*,
    crate::AsPtr,
    libc::{sockaddr_un, AF_UNIX},
    std::{
        io,
        mem::{transmute, zeroed},
        os::unix::net::SocketAddr,
    },
};

macro_rules! cfg_atomic_cloexec {
    ($($block:tt)+) => {
        #[cfg(any(
            // List taken from the standard library (std/src/sys/net/connection/socket/unix.rs)
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "illumos",
            target_os = "hurd",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
            target_os = "nto",
            target_os = "solaris",
        ))]
        $($block)+
    };
}
macro_rules! cfg_no_atomic_cloexec {
    ($($block:tt)+) => {
        #[cfg(not(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "illumos",
            target_os = "hurd",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
            target_os = "nto",
            target_os = "solaris",
        )))]
        $($block)+
    };
}

cfg_atomic_cloexec! {
    pub(super) unsafe fn fcntl_int(
        fd: BorrowedFd<'_>,
        cmd: c_int,
        val: c_int,
    ) -> io::Result<c_int> {
        unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) }.fd_or_errno()
    }
}

pub(super) fn duplicate_fd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    cfg_atomic_cloexec! {{
        let new_fd = unsafe { fcntl_int(fd, libc::F_DUPFD_CLOEXEC, 0)? };
        Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
    }}
    cfg_no_atomic_cloexec! {{
        let new_fd = unsafe {
            libc::dup(fd.as_raw_fd())
                .fd_or_errno()
                .map(|fd| OwnedFd::from_raw_fd(fd))?
        };
        set_cloexec(new_fd.as_fd())?;
        Ok(new_fd)
    }}
}

fn get_flflags(fd: BorrowedFd<'_>) -> io::Result<c_int> {
    unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL, 0) }.fd_or_errno()
}
fn set_flflags(fd: BorrowedFd<'_>, flags: c_int) -> io::Result<()> {
    unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags) != -1 }.true_val_or_errno(())
}
pub(super) fn set_nonblocking(fd: BorrowedFd<'_>, nonblocking: bool) -> io::Result<()> {
    let old_flags = get_flflags(fd)? & libc::O_NONBLOCK;
    set_flflags(fd, old_flags | if nonblocking { libc::O_NONBLOCK } else { 0 })
}

cfg_no_atomic_cloexec! {
    fn set_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) != -1 }.true_val_or_errno(())
    }
}

pub(super) fn set_socket_mode(fd: BorrowedFd<'_>, mode: mode_t) -> io::Result<()> {
    let rslt = unsafe { libc::fchmod(fd.as_raw_fd(), mode) != -1 }.true_val_or_errno(());
    if let Err(e) = &rslt {
        if e.kind() == io::ErrorKind::InvalidInput {
            return Err(io::Error::from(io::ErrorKind::Unsupported));
        }
    }
    rslt
}

/// Creates a Unix domain socket of the given type. If `nonblocking` is `true`, also makes it
/// nonblocking.
#[allow(unused_mut, unused_assignments)]
fn create_socket(ty: c_int, nonblocking: bool) -> io::Result<OwnedFd> {
    // Suppress warning on platforms that don't support the flag.
    let _ = nonblocking;
    let mut flags = 0;
    let mut can_create_nonblocking = false;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "openbsd",
    ))]
    {
        can_create_nonblocking = true;
        if nonblocking {
            flags |= libc::SOCK_NONBLOCK;
        }
    }
    cfg_atomic_cloexec! {{
        flags |= libc::SOCK_CLOEXEC;
    }}
    let fd = unsafe { libc::socket(AF_UNIX, ty | flags, 0) }
        .fd_or_errno()
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })?;
    cfg_no_atomic_cloexec! {{
        set_cloexec(fd.as_fd())?;
    }}

    if !can_create_nonblocking && nonblocking {
        set_nonblocking(fd.as_fd(), true)?;
    }

    Ok(fd)
}

fn addr_to_slice(addr: &SocketAddr) -> (&[u8], usize) {
    if let Some(slice) = addr.as_pathname() {
        (slice.as_os_str().as_bytes(), 0)
    } else {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(slice) = addr.as_abstract_name() {
            return (slice, 1);
        }
        (&[], 0)
    }
}

#[allow(clippy::as_conversions)]
const SUN_PATH_OFFSET: usize = unsafe {
    // This code may or may not have been copied from the standard library
    let addr = zeroed::<sockaddr_un>();
    let base = (&addr as *const sockaddr_un).cast::<libc::c_char>();
    let path = &addr.sun_path as *const c_char;
    path.byte_offset_from(base) as usize
};

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::as_conversions)]
fn bind(fd: BorrowedFd<'_>, addr: &SocketAddr) -> io::Result<()> {
    let (path, extra) = addr_to_slice(addr);
    let path = unsafe { transmute::<&[u8], &[libc::c_char]>(path) };

    let mut addr = unsafe { zeroed::<sockaddr_un>() };
    addr.sun_family = AF_UNIX as _;
    addr.sun_path[extra..(extra + path.len())].copy_from_slice(path);

    let len = path.len() + extra + SUN_PATH_OFFSET;

    unsafe {
        libc::bind(
            fd.as_raw_fd(),
            addr.as_ptr().cast(),
            // It's impossible for this to exceed socklen_t::MAX, since it came from a valid
            // SocketAddr
            len as _,
        ) != -1
    }
    .true_val_or_errno(())
}

fn listen(fd: BorrowedFd<'_>) -> io::Result<()> {
    // The standard library does this
    #[cfg(any(
        target_os = "windows",
        target_os = "redox",
        target_os = "espidf",
        target_os = "horizon"
    ))]
    const BACKLOG: libc::c_int = 128;
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "macos"
    ))]
    const BACKLOG: libc::c_int = -1;
    #[cfg(not(any(
        target_os = "windows",
        target_os = "redox",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "espidf",
        target_os = "horizon"
    )))]
    const BACKLOG: libc::c_int = libc::SOMAXCONN;
    unsafe { libc::listen(fd.as_raw_fd(), BACKLOG) != -1 }.true_val_or_errno(())
}

pub(super) fn create_server(
    ty: c_int,
    addr: &SocketAddr,
    nonblocking: bool,
    mode: Option<mode_t>,
) -> io::Result<OwnedFd> {
    let sock = create_socket(ty, nonblocking)?;
    if let Some(mode) = mode {
        // This used to forbid modes with the executable bit set, but no longer does. That is the
        // OS's business, not ours.
        set_socket_mode(sock.as_fd(), mode)?;
    }
    bind(sock.as_fd(), addr)?;
    listen(sock.as_fd())?;
    Ok(sock)
}

#[allow(dead_code)]
pub(super) fn shutdown(fd: BorrowedFd<'_>, how: std::net::Shutdown) -> io::Result<()> {
    use std::net::Shutdown::*;
    let how = match how {
        Read => libc::SHUT_RD,
        Write => libc::SHUT_WR,
        Both => libc::SHUT_RDWR,
    };
    unsafe { libc::shutdown(fd.as_raw_fd(), how) != -1 }.true_val_or_errno(())
}
