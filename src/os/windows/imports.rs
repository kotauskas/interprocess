#![allow(unused_imports, dead_code, unused_macros, non_camel_case_types)]
use cfg_if::cfg_if;

macro_rules! fake_consts {
    ($ty:ty, $($name:ident = $val:expr),+ $(,)?) => (
        $(
            pub(super) const $name : $ty = $val;
        )+
    );
}

cfg_if! {
    if #[cfg(windows)] {
        pub(super) use winapi::{
            shared::{minwindef::{DWORD, LPVOID}, ntdef::HANDLE, winerror::ERROR_PIPE_CONNECTED},
            um::{
                winbase::{
                    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_WRITE_THROUGH, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, PIPE_ACCESS_INBOUND,
                    PIPE_ACCESS_OUTBOUND, PIPE_READMODE_BYTE, PIPE_READMODE_MESSAGE,
                    PIPE_TYPE_BYTE, PIPE_TYPE_MESSAGE, PIPE_NOWAIT, PIPE_REJECT_REMOTE_CLIENTS,
                },
                winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE},
                fileapi::{CreateFileW, OPEN_EXISTING, FlushFileBuffers, ReadFile, WriteFile},
                handleapi::{CloseHandle, DuplicateHandle, INVALID_HANDLE_VALUE},
                namedpipeapi::{
                    ConnectNamedPipe, DisconnectNamedPipe,
                    PeekNamedPipe,
                    CreatePipe, CreateNamedPipeW, SetNamedPipeHandleState,
                },
                winbase::{
                    GetNamedPipeClientProcessId, GetNamedPipeClientSessionId,
                    GetNamedPipeServerProcessId, GetNamedPipeServerSessionId,
                },
                minwinbase::SECURITY_ATTRIBUTES,
                processthreadsapi::GetCurrentProcess,
            },
        };
        pub(super) use std::os::windows::{io::{AsRawHandle, FromRawHandle, IntoRawHandle}, ffi::{OsStrExt, OsStringExt}};
    } else {
        pub(super) type HANDLE = *mut std::ffi::c_void;
        pub trait AsRawHandle {}
        pub trait IntoRawHandle {}
        pub unsafe trait FromRawHandle {}
        pub(super) type DWORD = u32;
        pub struct SECURITY_ATTRIBUTES {}
        pub(super) type LPVOID = *mut std::ffi::c_void;

        fake_consts! {u32,
            PIPE_ACCESS_INBOUND = 0, PIPE_ACCESS_OUTBOUND = 1, PIPE_ACCESS_DUPLEX = 2,
            PIPE_TYPE_BYTE = 1, PIPE_TYPE_MESSAGE = 2,
            PIPE_READMODE_BYTE = 0, PIPE_READMODE_MESSAGE = 1,
        }
    }
}

cfg_if! {
    if #[cfg(feature = "tokio_support")] {
        pub use tokio::{
            io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf as TokioReadBuf},
        };
        pub use futures_io::{AsyncRead, AsyncWrite};
    }
}
cfg_if! {
    if #[cfg(all(windows, feature = "tokio_support"))] {
        pub(super) use tokio::net::windows::named_pipe::{
            NamedPipeClient as TokioNPClient,
            NamedPipeServer as TokioNPServer,
            ClientOptions as TokioNPClientOptions,
        };
    } else {
        #[derive(Debug)]
        pub struct TokioNPClient;
        #[derive(Debug)]
        pub struct TokioNPServer;
    }
}

cfg_if! {
    if #[cfg(all(windows, feature = "signals"))] {
        pub(super) use libc::{sighandler_t, SIGABRT, SIGFPE, SIGILL, SIGINT, SIGSEGV, SIGTERM};
        pub(super) use intmap::IntMap;
        pub(super) use once_cell::sync::Lazy;
        pub(super) use spinning::RwLock;
        pub(super) use thiserror::Error;

        // FIXME this is not yet in libc, remove when PR #1626 on rust-lang/libc gets merged
        pub const SIG_DFL: sighandler_t = 0;
    } else {
        fake_consts! {i32,
            SIGABRT = 100, SIGFPE = 101, SIGILL = 102, SIGINT = 103, SIGSEGV = 104, SIGTERM = 105,
        }
    }
}
