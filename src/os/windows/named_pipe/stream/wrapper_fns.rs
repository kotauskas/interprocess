use crate::os::windows::{imports::*, FileHandle};
use std::{io, os::windows::prelude::*, ptr};

/// Helper for several functions that take a handle and a DWORD out-pointer.
pub(crate) unsafe fn hget(
    handle: HANDLE,
    f: unsafe extern "system" fn(HANDLE, *mut DWORD) -> BOOL,
) -> io::Result<DWORD> {
    let mut x: u32 = 0;
    let ok = unsafe { f(handle, &mut x as *mut _) != 0 };
    ok_or_ret_errno!(ok => x)
}

pub(crate) fn get_flags(handle: HANDLE) -> io::Result<DWORD> {
    let mut flags: u32 = 0;
    let success = unsafe {
        GetNamedPipeInfo(
            handle,
            &mut flags as *mut _,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) != 0
    };
    ok_or_ret_errno!(success => flags)
}
pub(crate) fn is_server_from_sys(handle: HANDLE) -> io::Result<bool> {
    // Source: https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-getnamedpipeinfo
    const PIPE_IS_SERVER_BIT: u32 = 0x00000001;

    let flags = get_flags(handle)?;
    Ok(flags & PIPE_IS_SERVER_BIT != 0)
}
pub(crate) fn has_msg_boundaries_from_sys(handle: HANDLE) -> io::Result<bool> {
    // Same source.
    const PIPE_IS_MESSAGE_BIT: u32 = 0x00000004;

    let flags = get_flags(handle)?;
    Ok((flags & PIPE_IS_MESSAGE_BIT) != 0)
}
pub(crate) fn peek_msg_len(handle: HANDLE) -> io::Result<usize> {
    let mut len: DWORD = 0;
    let ok = unsafe {
        PeekNamedPipe(
            handle,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut len as *mut _,
        ) != 0
    };
    ok_or_ret_errno!(ok => len as usize)
}

pub(crate) fn _connect(
    path: &[u16],
    read: bool,
    write: bool,
    timeout: WaitTimeout,
) -> io::Result<FileHandle> {
    loop {
        match connect_without_waiting(path, read, write) {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                block_for_server(path, timeout)?;
                continue;
            }
            els => return els,
        }
    }
}

fn connect_without_waiting(path: &[u16], read: bool, write: bool) -> io::Result<FileHandle> {
    assert_eq!(path[path.len() - 1], 0, "nul terminator not found");
    let (success, handle) = unsafe {
        let handle = CreateFileW(
            path.as_ptr() as *mut _,
            {
                let mut access_flags: DWORD = 0;
                // TODO add FILE_READ_ATTRIBUTES perms
                if read {
                    access_flags |= GENERIC_READ;
                }
                if write {
                    access_flags |= GENERIC_WRITE;
                }
                access_flags
            },
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        (handle != INVALID_HANDLE_VALUE, handle)
    };
    if success {
        unsafe {
            // SAFETY: we just created this handle
            Ok(FileHandle::from_raw_handle(handle))
        }
    } else {
        Err(io::Error::last_os_error())
    }
}

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
