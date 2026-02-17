use {
    super::{downgrade_eof, winprelude::*},
    crate::{AsBuf, AsMutPtr as _, OrErrno as _, SubUsizeExt as _},
    std::{io, ptr},
    windows_sys::Win32::{
        Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, MAX_PATH},
        Storage::FileSystem::{FlushFileBuffers, GetFinalPathNameByHandleW, ReadFile, WriteFile},
        System::Threading::GetCurrentProcess,
    },
};

pub fn duplicate_handle(handle: BorrowedHandle<'_>) -> io::Result<OwnedHandle> {
    let raw = duplicate_handle_inner(handle, None)?;
    unsafe { Ok(OwnedHandle::from_raw_handle(raw.to_std())) }
}
pub fn duplicate_handle_to_foreign(
    handle: BorrowedHandle<'_>,
    other_process: BorrowedHandle<'_>,
) -> io::Result<HANDLE> {
    duplicate_handle_inner(handle, Some(other_process))
}

fn duplicate_handle_inner(
    handle: BorrowedHandle<'_>,
    other_process: Option<BorrowedHandle<'_>>,
) -> io::Result<HANDLE> {
    let mut new_handle = INVALID_HANDLE_VALUE;
    unsafe {
        let proc = GetCurrentProcess();
        DuplicateHandle(
            proc,
            handle.as_int_handle(),
            other_process.map(|h| h.as_int_handle()).unwrap_or(proc),
            &mut new_handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .true_val_or_errno(new_handle)
}

#[inline]
pub unsafe fn read_ptr(h: BorrowedHandle<'_>, ptr: *mut u8, len: usize) -> io::Result<usize> {
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    let mut bytes_read: u32 = 0;
    unsafe { ReadFile(h.as_int_handle(), ptr, len, bytes_read.as_mut_ptr(), ptr::null_mut()) }
        .true_val_or_errno(bytes_read.to_usize())
}
#[inline]
pub fn read(h: BorrowedHandle<'_>, buf: &mut (impl AsBuf + ?Sized)) -> io::Result<usize> {
    unsafe { read_ptr(h, buf.as_ptr(), buf.len()) }
}
#[inline]
pub fn write(h: BorrowedHandle<'_>, buf: &[u8]) -> io::Result<usize> {
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    let mut bytes_written: u32 = 0;
    unsafe {
        WriteFile(
            h.as_int_handle(),
            buf.as_ptr().cast(),
            len,
            bytes_written.as_mut_ptr(),
            ptr::null_mut(),
        )
    }
    .true_val_or_errno(bytes_written.to_usize())
}
#[inline]
pub fn flush(h: BorrowedHandle<'_>) -> io::Result<()> {
    downgrade_eof(unsafe { FlushFileBuffers(h.as_int_handle()) }.true_val_or_errno(()))
}

pub fn path(h: BorrowedHandle<'_>) -> io::Result<Vec<u16>> {
    let h = h.as_int_handle();
    let mut buf = Vec::with_capacity((MAX_PATH + 1).to_usize());
    loop {
        let bufcap = buf.capacity().try_into().unwrap_or(u32::MAX);
        let rslt = unsafe { GetFinalPathNameByHandleW(h, buf.as_mut_ptr(), bufcap, 0) };
        let false = rslt == 0 else { return Err(io::Error::last_os_error()) };
        if rslt == u32::MAX {
            return Err(io::Error::other("GetFinalPathNameByHandleW returned u32::MAX"));
        }
        #[allow(clippy::arithmetic_side_effects)] // cannot be u32::MAX
        let len_with_nul = rslt.to_usize() + 1;
        if rslt < bufcap {
            unsafe { buf.set_len(len_with_nul) };
            return Ok(buf);
        }
        buf.reserve_exact(len_with_nul);
    }
}
