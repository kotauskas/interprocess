use super::{c_wrappers, downgrade_eof, winprelude::*};
use crate::TryClone;
use std::{io, mem::MaybeUninit, ptr};
use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};

/// Newtype wrapper which defines file I/O operations on a handle to a file.
#[repr(transparent)]
pub(crate) struct FileHandle(OwnedHandle);
impl FileHandle {
    pub fn read(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);

        let (success, num_bytes_read) = unsafe {
            let mut num_bytes_read: u32 = 0;
            let result = ReadFile(
                self.as_int_handle(),
                buf.as_mut_ptr().cast(),
                len,
                &mut num_bytes_read as *mut _,
                ptr::null_mut(),
            );
            (result != 0, num_bytes_read as usize)
        };
        ok_or_ret_errno!(success => num_bytes_read)
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);

        let (success, bytes_written) = unsafe {
            let mut bytes_written: u32 = 0;
            let result = WriteFile(
                self.as_int_handle(),
                buf.as_ptr().cast(),
                len,
                &mut bytes_written as *mut _,
                ptr::null_mut(),
            );
            (result != 0, bytes_written as usize)
        };
        ok_or_ret_errno!(success => bytes_written)
    }
    #[inline(always)]
    pub fn flush(&self) -> io::Result<()> {
        Self::flush_hndl(self.as_int_handle())
    }
    #[inline]
    pub fn flush_hndl(handle: HANDLE) -> io::Result<()> {
        let success = unsafe { FlushFileBuffers(handle) != 0 };
        downgrade_eof(ok_or_ret_errno!(success => ()))
    }
}
impl TryClone for FileHandle {
    fn try_clone(&self) -> io::Result<Self> {
        c_wrappers::duplicate_handle(self.as_handle()).map(Self)
    }
}

multimacro! {
    FileHandle,
    forward_handle,
    forward_debug,
    derive_raw,
}
