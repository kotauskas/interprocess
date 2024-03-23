use super::{c_wrappers, unixprelude::*};
use crate::{OrErrno, TryClone};
use std::io::{self, prelude::*, IoSlice, IoSliceMut};

#[repr(transparent)]
pub(super) struct FdOps(pub(super) OwnedFd);
impl Read for &FdOps {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let length_to_read = buf.len();
		let bytes_read =
			unsafe { libc::read(self.0.as_raw_fd(), buf.as_mut_ptr().cast(), length_to_read) };
		(bytes_read >= 0).true_val_or_errno(bytes_read as usize)
	}
	fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
		let num_bufs = c_int::try_from(bufs.len()).unwrap_or(c_int::MAX);
		let bytes_read = unsafe { libc::readv(self.0.as_raw_fd(), bufs.as_ptr().cast(), num_bufs) };
		(bytes_read >= 0).true_val_or_errno(bytes_read as usize)
	}
	// FUTURE can_vector
}
impl Write for &FdOps {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let length_to_write = buf.len();
		let bytes_written =
			unsafe { libc::write(self.0.as_raw_fd(), buf.as_ptr().cast(), length_to_write) };
		(bytes_written >= 0).true_val_or_errno(bytes_written as usize)
	}
	fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
		let num_bufs = c_int::try_from(bufs.len()).unwrap_or(c_int::MAX);
		let bytes_written =
			unsafe { libc::writev(self.0.as_raw_fd(), bufs.as_ptr().cast(), num_bufs) };
		(bytes_written >= 0).true_val_or_errno(bytes_written as usize)
	}
	// FUTURE can_vector
	fn flush(&mut self) -> io::Result<()> {
		unsafe { libc::fsync(self.0.as_raw_fd()) >= 0 }.true_val_or_errno(())
	}
}

impl TryClone for FdOps {
	fn try_clone(&self) -> std::io::Result<Self> {
		let fd = c_wrappers::duplicate_fd(self.0.as_fd())?;
		Ok(Self(fd))
	}
}

multimacro! {
	FdOps,
	forward_handle,
	forward_debug,
	derive_raw,
}
