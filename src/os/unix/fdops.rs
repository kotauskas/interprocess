use super::{c_wrappers, unixprelude::*};
use crate::TryClone;
use std::{
    io::{self, prelude::*, IoSlice, IoSliceMut},
    os::fd::OwnedFd,
};
use to_method::To;

#[repr(transparent)]
pub(super) struct FdOps(pub(super) OwnedFd);
impl Read for &FdOps {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let length_to_read = buf.len();

        let (success, bytes_read) = unsafe {
            let size_or_err = libc::read(self.0.as_raw_fd(), buf.as_mut_ptr().cast(), length_to_read);
            (size_or_err >= 0, size_or_err as usize)
        };
        ok_or_ret_errno!(success => bytes_read)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let num_bufs = bufs.len().try_to::<c_int>().unwrap_or(c_int::MAX);

        let (success, bytes_read) = unsafe {
            let size_or_err = libc::readv(self.0.as_raw_fd(), bufs.as_ptr().cast(), num_bufs);
            (size_or_err >= 0, size_or_err as usize)
        };
        ok_or_ret_errno!(success => bytes_read)
    }
    // TODO can_vector
}
impl Write for &FdOps {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let length_to_write = buf.len();

        let (success, bytes_written) = unsafe {
            let size_or_err = libc::write(self.0.as_raw_fd(), buf.as_ptr().cast(), length_to_write);
            (size_or_err >= 0, size_or_err as usize)
        };
        ok_or_ret_errno!(success => bytes_written)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let num_bufs = bufs.len().try_to::<c_int>().unwrap_or(c_int::MAX);

        let (success, bytes_written) = unsafe {
            let size_or_err = libc::writev(self.0.as_raw_fd(), bufs.as_ptr().cast(), num_bufs);
            (size_or_err >= 0, size_or_err as usize)
        };
        ok_or_ret_errno!(success => bytes_written)
    }
    // TODO can_vector
    fn flush(&mut self) -> io::Result<()> {
        let success = unsafe { libc::fsync(self.0.as_raw_fd()) >= 0 };
        ok_or_ret_errno!(success => ())
    }
}

// No As/Into because those can be easily done with basic method forwarding.
impl FromRawFd for FdOps {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

impl TryClone for FdOps {
    fn try_clone(&self) -> std::io::Result<Self> {
        let fd = c_wrappers::duplicate_fd(self.0.as_fd())?;
        Ok(Self(fd))
    }
}
