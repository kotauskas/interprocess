use super::winprelude::*;
use crate::os::windows::get_borrowed;
use std::{
    io,
    mem::{size_of, zeroed},
};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS},
    Security::SECURITY_ATTRIBUTES,
    System::Threading::GetCurrentProcess,
};

pub fn duplicate_handle(handle: BorrowedHandle<'_>) -> io::Result<OwnedHandle> {
    let raw = duplicate_handle_inner(handle, None)?;
    unsafe { Ok(OwnedHandle::from_raw_handle(raw as RawHandle)) }
}
pub fn duplicate_handle_to_foreign(
    handle: BorrowedHandle<'_>,
    other_process: BorrowedHandle<'_>,
) -> io::Result<HANDLE> {
    duplicate_handle_inner(handle, Some(other_process))
}

fn duplicate_handle_inner(handle: BorrowedHandle<'_>, other_process: Option<BorrowedHandle<'_>>) -> io::Result<HANDLE> {
    let mut new_handle = INVALID_HANDLE_VALUE;
    let success = unsafe {
        let proc = GetCurrentProcess();
        DuplicateHandle(
            proc,
            get_borrowed(handle),
            other_process.map(|h| get_borrowed(h)).unwrap_or(proc),
            &mut new_handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        ) != 0
    };
    ok_or_ret_errno!(success => new_handle)
}

pub fn init_security_attributes() -> SECURITY_ATTRIBUTES {
    let mut a: SECURITY_ATTRIBUTES = unsafe { zeroed() };
    a.nLength = size_of::<SECURITY_ATTRIBUTES>() as _;
    a
}
