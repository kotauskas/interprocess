#[allow(unused_imports)]
use crate::{FdOrErrno, OrErrno};
use {
    super::unixprelude::*,
    crate::os::unix::ud_addr::TerminatedUdAddr,
    libc::{sockaddr_un, AF_UNIX},
    std::{ffi::CStr, io, mem::zeroed},
};

macro_rules! cfg_atomic_cloexec {
    ($($code:tt)+) => {
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
        $($code)+
    };
}
macro_rules! cfg_no_atomic_cloexec {
    ($($code:tt)+) => {
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
        $($code)+
    };
}

pub(super) unsafe fn fcntl_int(fd: BorrowedFd<'_>, cmd: c_int, val: c_int) -> io::Result<c_int> {
    unsafe { libc::fcntl(fd.as_raw_fd(), cmd, val) }.fd_or_errno()
}

fn get_flflags(fd: BorrowedFd<'_>) -> io::Result<c_int> {
    unsafe { fcntl_int(fd, libc::F_GETFL, 0) }
}
fn set_flflags(fd: BorrowedFd<'_>, flags: c_int) -> io::Result<()> {
    unsafe { fcntl_int(fd, libc::F_SETFL, flags) }.map(drop)
}
pub(super) fn set_nonblocking(fd: BorrowedFd<'_>, nonblocking: bool) -> io::Result<()> {
    let old_flags = get_flflags(fd)? & libc::O_NONBLOCK;
    set_flflags(fd, old_flags | if nonblocking { libc::O_NONBLOCK } else { 0 })
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

pub(super) fn unlink(path: &CStr) -> io::Result<()> {
    unsafe { libc::unlink(path.as_ptr()) != -1 }.true_val_or_errno(())
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

pub(super) unsafe fn stat_ptr(path: *const c_char) -> io::Result<libc::stat> {
    let mut rslt = unsafe { zeroed::<libc::stat>() };
    unsafe { libc::stat(path, &mut rslt) != -1 }.true_val_or_errno(rslt)
}

const NONBLOCKING_PARAMS: (bool, c_int) = {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "openbsd",
    ))]
    {
        (true, libc::SOCK_NONBLOCK)
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "openbsd",
    )))]
    {
        (false, 0)
    }
};
const CAN_CREATE_NONBLOCKING: bool = NONBLOCKING_PARAMS.0;
const NONBLOCKING_FLAG: c_int = NONBLOCKING_PARAMS.1;

/// Creates a Unix domain socket of the given type. If `nonblocking` and
/// [`CAN_CREATE_NONBLOCKING`] are both `true`, also makes it nonblocking.
#[allow(unused_mut)]
fn create_socket(ty: c_int, nonblocking: bool) -> io::Result<OwnedFd> {
    let mut flags = if nonblocking { NONBLOCKING_FLAG } else { 0 };
    cfg_atomic_cloexec! {{
        flags |= libc::SOCK_CLOEXEC;
    }}
    let fd = unsafe { libc::socket(AF_UNIX, ty | flags, 0) }
        .fd_or_errno()
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })?;
    cfg_no_atomic_cloexec! {{
        set_cloexec(fd.as_fd())?;
    }}

    if !CAN_CREATE_NONBLOCKING && nonblocking {
        set_nonblocking(fd.as_fd(), true)?;
    }

    Ok(fd)
}

#[allow(clippy::as_conversions)]
const SUN_PATH_OFFSET: usize = unsafe {
    // This code may or may not have been copied from the standard library
    let addr = zeroed::<sockaddr_un>();
    let base = (&addr as *const sockaddr_un).cast::<c_char>();
    let path = &addr.sun_path as *const c_char;
    path.byte_offset_from(base) as usize
};

fn bind(fd: BorrowedFd<'_>, addr: TerminatedUdAddr<'_>) -> io::Result<()> {
    unsafe { libc::bind(fd.as_raw_fd(), addr.addr_ptr().cast(), addr.addrlen()) != -1 }
        .true_val_or_errno(())
        .map_err(|e| {
            if e.raw_os_error() == Some(libc::EEXIST) {
                io::Error::from_raw_os_error(libc::EADDRINUSE)
            } else {
                e
            }
        })
}

fn listen(fd: BorrowedFd<'_>) -> io::Result<()> {
    // The standard library does this
    #[cfg(any(
        target_os = "windows",
        target_os = "redox",
        target_os = "espidf",
        target_os = "horizon"
    ))]
    const BACKLOG: c_int = 128;
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "macos"
    ))]
    const BACKLOG: c_int = -1;
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
    const BACKLOG: c_int = libc::SOMAXCONN;
    unsafe { libc::listen(fd.as_raw_fd(), BACKLOG) != -1 }.true_val_or_errno(())
}

pub(super) fn create_listener(
    ty: c_int,
    addr: TerminatedUdAddr<'_>,
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
    if !CAN_CREATE_NONBLOCKING && nonblocking {
        set_nonblocking(sock.as_fd(), true)?;
    }
    Ok(sock)
}

pub(super) fn connect(fd: BorrowedFd<'_>, addr: TerminatedUdAddr<'_>) -> io::Result<()> {
    unsafe { libc::connect(fd.as_raw_fd(), addr.addr_ptr().cast(), addr.addrlen()) != -1 }
        .true_val_or_errno(())
}

pub(super) fn create_client(
    dst: TerminatedUdAddr<'_>,
    nb_connect: bool,
    nb_stream: bool,
) -> io::Result<(OwnedFd, bool)> {
    let sock = create_socket(libc::SOCK_STREAM, nb_connect)?;
    if !CAN_CREATE_NONBLOCKING && nb_connect {
        set_nonblocking(sock.as_fd(), true)?;
    }
    let inprog = match connect(sock.as_fd(), dst) {
        Ok(()) => false,
        Err(e) if e.raw_os_error() == Some(libc::EINPROGRESS) => true,
        Err(e) => return Err(e),
    };
    if nb_connect != nb_stream {
        set_nonblocking(sock.as_fd(), nb_stream)?;
    }
    Ok((sock, inprog))
}
#[allow(dead_code)]
pub(super) fn create_client_nonblockingly(
    dst: TerminatedUdAddr<'_>,
    ret_nonblocking: bool,
) -> io::Result<(OwnedFd, bool)> {
    let sock = create_socket(libc::SOCK_STREAM, true)?;
    if !CAN_CREATE_NONBLOCKING {
        set_nonblocking(sock.as_fd(), true)?;
    }
    let inprog = match connect(sock.as_fd(), dst) {
        Ok(()) => false,
        Err(e) if e.raw_os_error() == Some(libc::EINPROGRESS) => true,
        Err(e) => return Err(e),
    };
    if !ret_nonblocking {
        set_nonblocking(sock.as_fd(), false)?;
    }
    Ok((sock, inprog))
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
