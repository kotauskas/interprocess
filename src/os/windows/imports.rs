#![allow(unused_imports, dead_code, non_camel_case_types)]
use std::ffi::c_void;

#[cfg(windows)]
pub(super) use {
    std::os::windows::ffi::{OsStrExt, OsStringExt},
    winapi::{
        shared::winerror::{ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_MORE_DATA},
        um::{
            fileapi::{CreateFileW, FlushFileBuffers, ReadFile, WriteFile, OPEN_EXISTING},
            handleapi::{CloseHandle, DuplicateHandle, INVALID_HANDLE_VALUE},
            namedpipeapi::{
                ConnectNamedPipe, CreateNamedPipeW, CreatePipe, DisconnectNamedPipe, GetNamedPipeHandleStateW,
                GetNamedPipeInfo, PeekNamedPipe, SetNamedPipeHandleState, WaitNamedPipeW,
            },
            processthreadsapi::GetCurrentProcess,
            winbase::{
                GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, GetNamedPipeServerProcessId,
                GetNamedPipeServerSessionId,
            },
            winbase::{
                FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_FLAG_WRITE_THROUGH, PIPE_NOWAIT,
                PIPE_REJECT_REMOTE_CLIENTS,
            },
            winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE},
        },
    },
};

import_type_alias_or_make_dummy!(types {winapi::shared::minwindef}::(
    DWORD  = u32,
    LPVOID = *mut c_void,
    BOOL   = i32,
), cfg(windows));
import_type_alias_or_make_dummy!(type {winapi::shared::ntdef}::HANDLE = *mut c_void, cfg(windows));
import_type_or_make_dummy!(type {winapi::um::minwinbase}::SECURITY_ATTRIBUTES, cfg(windows));

import_const_or_make_dummy!(u32: consts {winapi::um::winbase}::(
    PIPE_ACCESS_INBOUND = 0, PIPE_ACCESS_OUTBOUND = 1, PIPE_ACCESS_DUPLEX = 2,
    PIPE_TYPE_BYTE = 1, PIPE_TYPE_MESSAGE = 2,
    PIPE_READMODE_BYTE = 0, PIPE_READMODE_MESSAGE = 1,
), cfg(windows));

import_trait_or_make_dummy!(traits {std::os::windows::io}::(
    AsRawHandle, IntoRawHandle, FromRawHandle,
), cfg(windows));

import_trait_or_make_dummy!(traits {futures_io}::(
    AsyncRead, AsyncWrite,
), cfg(feature = "tokio"));
import_trait_or_make_dummy!(traits {tokio::io}::(
    AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite,
), cfg(feature = "tokio"));
import_type_or_make_dummy!(
    type {tokio::io}::ReadBuf as TokioReadBuf<'a>, cfg(feature = "tokio"),
);

#[cfg(all(windows, feature = "tokio"))]
pub(super) use tokio::net::windows::named_pipe::ClientOptions as TokioNPClientOptions;

import_type_or_make_dummy!(types {tokio::net::windows::named_pipe}::(
    NamedPipeClient as TokioNPClient,
    NamedPipeServer as TokioNPServer,
), cfg(all(windows, feature = "tokio")));
import_type_or_make_dummy!(types {tokio::task}::(
    JoinHandle as TokioJoinHandle,
), cfg(all(windows, feature = "tokio")));
import_type_or_make_dummy!(types {tokio::sync}::(
    Mutex as TokioMutex,
    MutexGuard as TokioMutexGuard,
), cfg(all(windows, feature = "tokio")));

#[cfg(feature = "tokio")]
pub(super) use futures_core::ready;
