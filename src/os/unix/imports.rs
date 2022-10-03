#![allow(dead_code, unused_imports, non_camel_case_types)]
use cfg_if::cfg_if;

import_type_alias_or_make_dummy!(types {libc}::(
    c_int = i32,
    pid_t = i32,
    uid_t = i32,
    gid_t = i32,
    mode_t = u32,
    size_t = usize,
), cfg(unix));
import_type_alias_or_make_dummy!(type {super}::FdOps = (), cfg(unix));

import_trait_or_make_dummy!(traits {std::os::unix::io}::(
    AsRawFd, IntoRawFd, FromRawFd,
), cfg(unix));
import_trait_or_make_dummy!(traits {std::os::unix::ffi}::(
    OsStrExt, OsStringExt,
), cfg(unix));

import_type_or_make_dummy!(types {libc}::(
    sockaddr_un,
    msghdr,
    cmsghdr,
), cfg(uds_supported));
import_type_or_make_dummy!(types {std::os::unix::net}::(
    UnixStream as StdUdStream,
    UnixListener as StdUdStreamListener,
    UnixDatagram as StdUdSocket,
), cfg(uds_supported));

#[cfg(not(unix))]
pub(super) const _MAX_UDSOCKET_PATH_LEN: usize = 0;

#[cfg(uds_supported)]
pub(super) use libc::{
    iovec, sockaddr, socklen_t, AF_UNIX, FD_CLOEXEC, F_GETFD, F_GETFL, F_SETFD, F_SETFL,
    O_NONBLOCK, SHUT_RD, SHUT_RDWR, SHUT_WR, SOCK_DGRAM, SOCK_STREAM, SOL_SOCKET,
};

cfg_if! {
    if #[cfg(uds_ucred)] {
        pub(super) use libc::ucred;
    } else if #[cfg(uds_xucred)] {
        pub(super) use libc::xucred;
        pub struct ucred {}
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

#[cfg(se_basic)]
pub(super) use libc::{sigaction, SA_NOCLDSTOP, SA_NODEFER, SA_RESETHAND, SA_RESTART, SIG_DFL};

import_const_or_make_dummy!(i32: consts {libc}::(
    SIGHUP  = 0, SIGINT  = 1, SIGQUIT = 2, SIGILL  = 3,
    SIGABRT = 4, SIGFPE  = 5, SIGKILL = 6, SIGSEGV = 7,
    SIGPIPE = 8, SIGALRM = 9, SIGTERM = 10,
), cfg(se_basic));

import_const_or_make_dummy!(i32: consts {libc}::(
    SIGUSR1 = 11, SIGUSR2 = 12, SIGCHLD = 13, SIGCONT = 14,
    SIGSTOP = 15, SIGTSTP = 16, SIGTTIN = 17, SIGTTOU = 18,
), cfg(se_full_posix_1990));

import_const_or_make_dummy!(i32: consts {libc}::(
    SIGBUS    = 19, SIGURG  = 20,
    SIGPROF   = 21, SIGSYS  = 22, SIGTRAP = 23,
    SIGVTALRM = 24, SIGXCPU = 25, SIGXFSZ = 26,
), cfg(se_base_posix_2001));

cfg_if! {
    if #[cfg(se_sigpoll)] {
        pub(super) use libc::SIGPOLL;
    } else if #[cfg(se_sigpoll_is_sigio)] {
        pub(super) use libc::SIGIO as SIGPOLL;
    } else {
        const SIGPOLL: i32 = 27;
    }
}

import_type_or_make_dummy!(types {tokio::net}::(
    UnixListener as TokioUdStreamListener,
    UnixStream as TokioUdStream,
    UnixDatagram as TokioUdSocket,
), cfg(all(uds_supported, feature = "tokio_support")));
import_type_or_make_dummy!(types {tokio::net::unix}::(
    ReadHalf as TokioUdStreamReadHalf<'a>,
    OwnedReadHalf as TokioUdStreamOwnedReadHalf,
    WriteHalf as TokioUdStreamWriteHalf<'a>,
    OwnedWriteHalf as TokioUdStreamOwnedWriteHalf,
), cfg(all(unix, feature = "tokio_support")));

#[cfg(all(unix, feature = "tokio_support"))]
pub use tokio::net::unix::ReuniteError as TokioReuniteError;
#[cfg(not(all(unix, feature = "tokio_support")))]
pub struct TokioReuniteError(pub (), pub ());

import_type_or_make_dummy!(type {tokio::io}::ReadBuf<'a>, cfg(feature = "tokio_support"));

import_trait_or_make_dummy!(traits {tokio::io}::(
    AsyncRead as TokioAsyncRead,
    AsyncWrite as TokioAsyncWrite,
), cfg(feature = "tokio_support"));

import_trait_or_make_dummy!(traits {futures_io}::(
    AsyncRead as FuturesAsyncRead,
    AsyncWrite as FuturesAsyncWrite,
), cfg(feature = "tokio_support"));
