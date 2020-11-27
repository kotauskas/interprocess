use cfg_if::cfg_if;

#[allow(unused_macros)]
macro_rules! fake_signals {
    ($($name:ident = $val:expr),+ $(,)?) => (
        $(
            #[cfg(not(unix))]
            const $name : i32 = $val;
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
            SIGSEGV, SIGPOLL  ,
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
            SO_PASSCRED,
            SCM_RIGHTS, SCM_CREDENTIALS,
            MSG_TRUNC, MSG_CTRUNC, MSG_PEEK,
            sockaddr_un,
            msghdr, cmsghdr,
            ucred,
        };

        #[cfg(not(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "android",
            target_os = "haiku",
            target_os = "fuchsia",
            target_os = "solaris",
            target_os = "illumos",
        )))]
        fake_signals!(SIGPOLL = 999); // Assign an arbitrary number to get doctests to compile.

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
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub type c_int = i32;
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub type pid_t = i32;
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub type uid_t = i32;
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub type gid_t = i32;
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub type mode_t = u32;

        pub(super) const _MAX_UDSOCKET_PATH_LEN: usize = 0;

        type FdOps = ();
    }
}