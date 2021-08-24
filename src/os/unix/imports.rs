#![allow(dead_code, unused_imports, non_camel_case_types)]

use cfg_if::cfg_if;

#[allow(unused_macros)]
macro_rules! fake_signals {
    ($($name:ident = $val:expr),+ $(,)?) => (
        $(
            #[cfg(not(unix))]
            pub(super) const $name : i32 = $val;
        )+
    );
}

cfg_if! {
    if #[cfg(unix)] {
        pub(super) use libc::{c_int, pid_t, uid_t, gid_t, mode_t, size_t};
        pub(super) use super::FdOps;
        pub (super) use std::os::unix::{
            io::{AsRawFd, IntoRawFd, FromRawFd},
            ffi::{OsStrExt, OsStringExt},
        };
    } else {
        pub type c_int = i32;
        pub type pid_t = i32;
        pub type uid_t = i32;
        pub type gid_t = i32;
        pub type mode_t = u32;
        pub type size_t = usize;
        pub(super) const _MAX_UDSOCKET_PATH_LEN: usize = 0;
        pub(super) type FdOps = ();
    }
}

cfg_if! {
    if #[cfg(uds_supported)] {
        pub(super) use libc::{
            sockaddr_un, sockaddr,
            msghdr, cmsghdr,
            socklen_t, iovec,
            MSG_TRUNC, MSG_CTRUNC,
            AF_UNIX, SOCK_STREAM, SOCK_DGRAM, SOL_SOCKET,
            O_NONBLOCK, F_GETFL, F_SETFL,
            SHUT_RD, SHUT_WR, SHUT_RDWR,
        };
    } else {
        pub struct sockaddr_un {}
        pub struct msghdr {}
    }
}
cfg_if! {
    if #[cfg(uds_ucred)] {
        pub(super) use libc::ucred;
    } else if #[cfg(uds_xucred)] {
        pub(super) use libc::xucred;
    } else {
        pub struct ucred {}
    }
}
#[cfg(uds_scm_rights)]
pub(super) use libc::SCM_RIGHTS;
#[cfg(uds_peercred)]
pub(super) use libc::SO_PEERCRED;
#[cfg(uds_scm_credentials)]
pub(super) use libc::{SCM_CREDENTIALS, SO_PASSCRED};

#[cfg(feature = "signals")]
pub(super) use {intmap::IntMap, once_cell::sync::Lazy, spinning::RwLock, thiserror::Error};
#[cfg(feature = "signals")]
cfg_if! {
    if #[cfg(se_basic)] {
        pub(super) use libc::{
            SIGHUP , SIGINT , SIGQUIT, SIGILL ,
            SIGABRT, SIGFPE , SIGKILL, SIGSEGV,
            SIGPIPE, SIGALRM, SIGTERM,

            SIG_DFL,
            SA_NOCLDSTOP, SA_NODEFER, SA_RESETHAND, SA_RESTART,
            sigaction,
        };
    } else {
        fake_signals! {
            SIGHUP  = 0, SIGINT  = 1, SIGQUIT = 2, SIGILL  = 3,
            SIGABRT = 4, SIGFPE  = 5, SIGKILL = 6, SIGSEGV = 7,
            SIGPIPE = 8, SIGALRM = 9, SIGTERM = 10,
        }
    }
}
#[cfg(feature = "signals")]
cfg_if! {
    if #[cfg(se_full_posix_1990)] {
        pub(super) use libc::{
            SIGUSR1, SIGUSR2, SIGCHLD, SIGCONT,
            SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU,
        };
    } else {
        fake_signals! {
            SIGUSR1 = 11, SIGUSR2 = 12, SIGCHLD = 13, SIGCONT = 14,
            SIGSTOP = 15, SIGTSTP = 16, SIGTTIN = 17, SIGTTOU = 18,
        }
    }
}
#[cfg(feature = "signals")]
cfg_if! {
    if #[cfg(se_base_posix_2001)] {
        pub(super) use libc::{
            SIGBUS   , SIGURG ,
            SIGPROF  , SIGSYS , SIGTRAP,
            SIGVTALRM, SIGXCPU, SIGXFSZ,
        };
    } else {
        fake_signals! {
            SIGBUS    = 19, SIGURG  = 20,
            SIGPROF   = 21, SIGSYS  = 22, SIGTRAP = 23,
            SIGVTALRM = 24, SIGXCPU = 25, SIGXFSZ = 26,
        }
    }
}
#[cfg(feature = "signals")]
cfg_if! {
    if #[cfg(se_sigpoll)] {
        pub(super) use libc::SIGPOLL;
    } else if #[cfg(se_sigpoll_is_sigio)] {
        pub(super) use libc::SIGIO as SIGPOLL;
    } else {
        fake_signals!(SIGPOLL = 27);
    }
}
