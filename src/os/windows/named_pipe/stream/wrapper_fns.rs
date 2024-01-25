use crate::os::windows::{named_pipe::PipeMode, winprelude::*, FileHandle};
use std::{io, mem::MaybeUninit, os::windows::prelude::*, ptr};
use winapi::{
    shared::winerror::ERROR_PIPE_BUSY,
    um::{
        fileapi::{CreateFileW, OPEN_EXISTING},
        handleapi::INVALID_HANDLE_VALUE,
        namedpipeapi::{
            GetNamedPipeHandleStateW, GetNamedPipeInfo, PeekNamedPipe, SetNamedPipeHandleState, WaitNamedPipeW,
        },
        winbase::{FILE_FLAG_OVERLAPPED, PIPE_NOWAIT},
        winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, GENERIC_READ, GENERIC_WRITE},
    },
};

/// Helper for several functions that take a handle and a DWORD out-pointer.
pub(crate) unsafe fn hget(
    handle: BorrowedHandle<'_>,
    f: unsafe extern "system" fn(HANDLE, *mut DWORD) -> BOOL,
) -> io::Result<DWORD> {
    let mut x: u32 = 0;
    let ok = unsafe { f(handle.as_raw_handle(), &mut x as *mut _) != 0 };
    ok_or_ret_errno!(ok => x)
}

pub(crate) fn get_flags(handle: BorrowedHandle<'_>) -> io::Result<DWORD> {
    let mut flags: u32 = 0;
    let success = unsafe {
        GetNamedPipeInfo(
            handle.as_raw_handle(),
            &mut flags as *mut _,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) != 0
    };
    ok_or_ret_errno!(success => flags)
}
pub(crate) fn is_server_from_sys(handle: BorrowedHandle<'_>) -> io::Result<bool> {
    // Source: https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-getnamedpipeinfo
    const PIPE_IS_SERVER_BIT: u32 = 0x00000001;

    let flags = get_flags(handle)?;
    Ok(flags & PIPE_IS_SERVER_BIT != 0)
}
pub(crate) fn has_msg_boundaries_from_sys(handle: BorrowedHandle<'_>) -> io::Result<bool> {
    // Same source.
    const PIPE_IS_MESSAGE_BIT: u32 = 0x00000004;

    let flags = get_flags(handle)?;
    Ok((flags & PIPE_IS_MESSAGE_BIT) != 0)
}
#[allow(dead_code)] // TODO give this thing a public API
pub(crate) fn peek_msg_len(handle: BorrowedHandle<'_>) -> io::Result<usize> {
    let mut msglen: DWORD = 0;
    let ok = unsafe {
        PeekNamedPipe(
            handle.as_raw_handle(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut msglen as *mut _,
        ) != 0
    };
    ok_or_ret_errno!(ok => msglen as usize)
}

/// This is used by sync named pipes only. Tokio ones call connect_without_waiting() directly.
pub(crate) fn _connect(
    path: &[u16],
    read: Option<PipeMode>,
    write: Option<PipeMode>,
    timeout: WaitTimeout,
) -> io::Result<FileHandle> {
    loop {
        match connect_without_waiting(path, read, write, false) {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as _) => {
                block_for_server(path, timeout)?;
                continue;
            }
            els => return els,
        }
    }
}

fn modes_to_access_flags(read: Option<PipeMode>, write: Option<PipeMode>) -> DWORD {
    let mut access_flags = 0;
    if read.is_some() {
        access_flags |= GENERIC_READ;
    }
    if read == Some(PipeMode::Messages) {
        access_flags |= FILE_WRITE_ATTRIBUTES;
    }
    if write.is_some() {
        access_flags |= GENERIC_WRITE;
    }
    access_flags
}

pub(crate) fn connect_without_waiting(
    path: &[u16],
    read: Option<PipeMode>,
    write: Option<PipeMode>,
    overlapped: bool,
) -> io::Result<FileHandle> {
    assert_eq!(path[path.len() - 1], 0, "nul terminator not found");
    let access_flags = modes_to_access_flags(read, write);
    let flags = if overlapped { FILE_FLAG_OVERLAPPED } else { 0 };
    let (success, handle) = unsafe {
        let handle = CreateFileW(
            path.as_ptr().cast(),
            access_flags,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            flags,
            ptr::null_mut(),
        );
        (handle != INVALID_HANDLE_VALUE, handle)
    };
    ok_or_ret_errno!(success => unsafe {
        // SAFETY: we just created this handle
        FileHandle(OwnedHandle::from_raw_handle(handle))
    })
}

#[allow(dead_code)]
pub(crate) fn get_named_pipe_handle_state(
    handle: BorrowedHandle<'_>,
    mode: Option<&mut DWORD>,
    cur_instances: Option<&mut DWORD>,
    max_collection_count: Option<&mut DWORD>,
    collect_data_timeout: Option<&mut DWORD>,
    mut username: Option<&mut [MaybeUninit<u16>]>,
) -> io::Result<()> {
    // TODO expose the rest of the owl as public API
    let toptr = |r: &mut DWORD| r as *mut DWORD;
    let null = ptr::null_mut();
    let success = unsafe {
        GetNamedPipeHandleStateW(
            handle.as_raw_handle(),
            mode.map(toptr).unwrap_or(null),
            cur_instances.map(toptr).unwrap_or(null),
            max_collection_count.map(toptr).unwrap_or(null),
            collect_data_timeout.map(toptr).unwrap_or(null),
            username
                .as_deref_mut()
                .map(|s| s.as_mut_ptr().cast())
                .unwrap_or(ptr::null_mut()),
            username
                .map(|s| DWORD::try_from(s.len()).unwrap_or(DWORD::MAX))
                .unwrap_or(0),
        ) != 0
    };
    ok_or_ret_errno!(success => ())
}
pub(crate) fn set_named_pipe_handle_state(
    handle: BorrowedHandle<'_>,
    mode: Option<DWORD>,
    max_collection_count: Option<DWORD>,
    collect_data_timeout: Option<DWORD>,
) -> io::Result<()> {
    let (mut mode_, has_mode) = (mode.unwrap_or_default(), mode.is_some());
    let (mut mcc, has_mcc) = (max_collection_count.unwrap_or_default(), max_collection_count.is_some());
    let (mut cdt, has_cdt) = (collect_data_timeout.unwrap_or_default(), collect_data_timeout.is_some());
    let toptr = |r: &mut DWORD| r as *mut DWORD;
    let null = ptr::null_mut();
    let success = unsafe {
        SetNamedPipeHandleState(
            handle.as_raw_handle(),
            if has_mode { toptr(&mut mode_) } else { null },
            if has_mcc { toptr(&mut mcc) } else { null },
            if has_cdt { toptr(&mut cdt) } else { null },
        ) != 0
    };
    ok_or_ret_errno!(success => ())
}

pub(crate) fn set_nonblocking_given_readmode(
    handle: BorrowedHandle<'_>,
    nonblocking: bool,
    read: Option<PipeMode>,
) -> io::Result<()> {
    // PIPE_READMODE_BYTE is the default
    let mut mode = read.unwrap_or(PipeMode::Bytes).to_readmode();
    if nonblocking {
        mode |= PIPE_NOWAIT;
    }
    set_named_pipe_handle_state(handle, Some(mode), None, None)
}

// TODO this should be public API
#[repr(transparent)] // #[repr(DWORD)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct WaitTimeout(u32);
impl WaitTimeout {
    pub(crate) const DEFAULT: Self = Self(0x00000000);
    //pub(crate) const FOREVER: Self = Self(0xffffffff);
}
impl From<WaitTimeout> for u32 {
    fn from(x: WaitTimeout) -> Self {
        x.0
    }
}
impl Default for WaitTimeout {
    fn default() -> Self {
        Self::DEFAULT
    }
}
pub(crate) fn block_for_server(path: &[u16], timeout: WaitTimeout) -> io::Result<()> {
    assert_eq!(path[path.len() - 1], 0, "nul terminator not found");
    let success = unsafe { WaitNamedPipeW(path.as_ptr() as *mut _, timeout.0) != 0 };
    ok_or_ret_errno!(success => ())
}
