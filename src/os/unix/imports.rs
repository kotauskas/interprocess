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
        pub(super) use libc::{
            SIGHUP , SIGCONT  ,
            SIGINT , SIGSTOP  ,
            SIGQUIT, SIGTSTP  ,
            SIGILL , SIGTTIN  ,
            SIGABRT, SIGTTOU  ,
            SIGFPE , SIGBUS   ,
            SIGKILL, SIGPROF  ,
            SIGSEGV, // no SIGPOLL because it doesn't exist on Apple platforms
            SIGPIPE, SIGSYS   ,
            SIGALRM, SIGTRAP  ,
            SIGTERM, SIGURG   ,
            SIGUSR1, SIGVTALRM,
            SIGUSR2, SIGXCPU  ,
            SIGCHLD, SIGXFSZ  ,
            SIG_DFL,
            SA_NOCLDSTOP, SA_NODEFER, SA_RESETHAND, SA_RESTART,
            sigaction,
            c_int,
            pid_t, uid_t, gid_t,
            mode_t,
            AF_UNIX,
            SOCK_STREAM, SOCK_DGRAM,
            SOL_SOCKET,
            SCM_RIGHTS,
            MSG_TRUNC, MSG_CTRUNC,
            F_GETFL,
            F_SETFL,
            O_NONBLOCK,
            sockaddr_un, sockaddr,
            msghdr, cmsghdr,
            socklen_t, size_t, iovec,
        };
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        pub(super) use libc::{
            SIGPOLL,
            SO_PASSCRED,
            SO_PEERCRED,
            SCM_CREDENTIALS,
            ucred,
        };

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        pub(super) const SIGPOLL: i32 = 999;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        #[doc(hidden)]
        pub struct ucred {}

        pub(super) use super::FdOps;

        pub (super) use std::os::unix::{
            io::{AsRawFd, IntoRawFd, FromRawFd},
            ffi::{OsStrExt, OsStringExt},
        };
    } else {
        fake_signals! {
            SIGHUP  = 0 , SIGCONT   = 14,
            SIGINT  = 1 , SIGSTOP   = 15,
            SIGQUIT = 2 , SIGTSTP   = 16,
            SIGILL  = 3 , SIGTTIN   = 17,
            SIGABRT = 4 , SIGTTOU   = 18,
            SIGFPE  = 5 , SIGBUS    = 19,
            SIGKILL = 6 , SIGPROF   = 20,
            SIGSEGV = 7 , SIGPOLL   = 21,
            SIGPIPE = 8 , SIGSYS    = 22,
            SIGALRM = 9 , SIGTRAP   = 23,
            SIGTERM = 10, SIGURG    = 24,
            SIGUSR1 = 11, SIGVTALRM = 25,
            SIGUSR2 = 12, SIGXCPU   = 26,
            SIGCHLD = 13, SIGXFSZ   = 27,
        }
        pub type c_int = i32;
        pub type pid_t = i32;
        pub type uid_t = i32;
        pub type gid_t = i32;
        pub type mode_t = u32;

        pub struct ucred {}
        pub struct sockaddr_un {}
        pub struct msghdr {}

        pub(super) const _MAX_UDSOCKET_PATH_LEN: usize = 0;

        pub(super) type FdOps = ();
    }
}

cfg_if! {
    if #[cfg(feature = "signals")] {
        pub use intmap::IntMap;
        pub use once_cell::sync::Lazy;
        pub use spinning::RwLock;
        pub use thiserror::Error;
    }
}
