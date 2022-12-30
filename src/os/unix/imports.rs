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

import_type_or_make_dummy!(types {tokio::net}::(
    UnixListener as TokioUdStreamListener,
    UnixStream as TokioUdStream,
    UnixDatagram as TokioUdSocket,
), cfg(all(uds_supported, feature = "tokio")));
import_type_or_make_dummy!(types {tokio::net::unix}::(
    ReadHalf as TokioUdStreamReadHalf<'a>,
    OwnedReadHalf as TokioUdStreamOwnedReadHalf,
    WriteHalf as TokioUdStreamWriteHalf<'a>,
    OwnedWriteHalf as TokioUdStreamOwnedWriteHalf,
), cfg(all(unix, feature = "tokio")));

#[cfg(all(unix, feature = "tokio"))]
pub use tokio::net::unix::ReuniteError as TokioReuniteError;
#[cfg(not(all(unix, feature = "tokio")))]
pub struct TokioReuniteError(pub (), pub ());

import_type_or_make_dummy!(type {tokio::io}::ReadBuf<'a>, cfg(feature = "tokio"));

import_trait_or_make_dummy!(traits {tokio::io}::(
    AsyncRead as TokioAsyncRead,
    AsyncWrite as TokioAsyncWrite,
), cfg(feature = "tokio"));

import_trait_or_make_dummy!(traits {futures_io}::(
    AsyncRead as FuturesAsyncRead,
    AsyncWrite as FuturesAsyncWrite,
), cfg(feature = "tokio"));
