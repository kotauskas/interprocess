use super::{c_wrappers, downgrade_eof, winprelude::*};
use crate::{OrErrno, TryClone};
use std::{io, mem::MaybeUninit, ptr};
use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};

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
				&mut bytes_read as *mut _,
				ptr::null_mut(),
			)
		}
		.true_val_or_errno(bytes_read as usize)
	}
	pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
		let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);

		let mut bytes_written: u32 = 0;
		unsafe {
			WriteFile(
				self.as_int_handle(),
				buf.as_ptr().cast(),
				len,
				&mut bytes_written as *mut _,
				ptr::null_mut(),
			)
		}
		.true_val_or_errno(bytes_written as usize)
	}
	#[inline(always)]
	pub fn flush(&self) -> io::Result<()> {
		Self::flush_hndl(self.as_int_handle())
	}
	#[inline]
	pub fn flush_hndl(handle: HANDLE) -> io::Result<()> {
		downgrade_eof(unsafe { FlushFileBuffers(handle) }.true_val_or_errno(()))
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
