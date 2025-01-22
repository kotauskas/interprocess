use {
    super::{c_wrappers, downgrade_eof, winprelude::*},
    crate::{AsMutPtr, OrErrno, SubUsizeExt, TryClone},
    std::{io, mem::MaybeUninit, ptr},
    windows_sys::Win32::{
        Foundation::MAX_PATH,
        Storage::FileSystem::{FlushFileBuffers, GetFinalPathNameByHandleW, ReadFile, WriteFile},
    },
};

/// Newtype wrapper which defines file I/O operations on a handle to a file.
#[repr(transparent)]
pub(crate) struct FileHandle(OwnedHandle);
impl FileHandle {
    pub fn read(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);

        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(
                self.as_int_handle(),
                buf.as_mut_ptr().cast(),
                len,
                bytes_read.as_mut_ptr(),
                ptr::null_mut(),
            )
        }
        .true_val_or_errno(bytes_read.to_usize())
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);

        let mut bytes_written: u32 = 0;
        unsafe {
            WriteFile(
                self.as_int_handle(),
                buf.as_ptr().cast(),
                len,
                bytes_written.as_mut_ptr(),
                ptr::null_mut(),
            )
        }
        .true_val_or_errno(bytes_written.to_usize())
    }
    #[inline(always)]
    pub fn flush(&self) -> io::Result<()> { Self::flush_hndl(self.as_int_handle()) }
    #[inline]
    pub fn flush_hndl(handle: HANDLE) -> io::Result<()> {
        downgrade_eof(unsafe { FlushFileBuffers(handle) }.true_val_or_errno(()))
    }

    // The second arm is unreachable if cap > len.
    #[allow(dead_code, clippy::arithmetic_side_effects)]
    pub fn path(handle: BorrowedHandle<'_>) -> io::Result<Vec<u16>> {
        let mut buf = Vec::with_capacity((MAX_PATH + 1).to_usize());
        match Self::_path(handle.as_int_handle(), &mut buf) {
            (_, Ok(true)) => Ok(buf),
            (len, Ok(false)) => {
                buf.reserve_exact(len - buf.capacity());
                match Self::_path(handle.as_int_handle(), &mut buf) {
                    (_, Ok(true)) => Ok(buf),
                    (_, Ok(false)) => unreachable!(),
                    (_, Err(e)) => Err(e),
                }
            }
            (_, Err(e)) => Err(e),
        }
    }
    #[allow(clippy::arithmetic_side_effects)] // Path lengths can never overflow usize.
    fn _path(handle: HANDLE, buf: &mut Vec<u16>) -> (usize, io::Result<bool>) {
        buf.clear();
        let buflen = buf.capacity().try_into().unwrap_or(u32::MAX);
        let rslt = unsafe { GetFinalPathNameByHandleW(handle, buf.as_mut_ptr(), buflen, 0) };
        let len = rslt.to_usize();
        let e = if rslt >= buflen {
            Ok(false)
        } else if rslt == 0 {
            Err(io::Error::last_os_error())
        } else {
            // +1 to include the nul terminator in the size.
            unsafe { buf.set_len(rslt.to_usize() + 1) }
            Ok(true)
        };
        (len, e)
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
