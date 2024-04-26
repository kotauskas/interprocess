pub(super) mod winprelude {
	pub(crate) use super::{AsRawHandleExt as _, HANDLEExt as _};
	pub(crate) use std::os::windows::prelude::*;
	pub(crate) use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
}

use std::io::{self, ErrorKind::BrokenPipe};
use winprelude::*;

use crate::RawOsErrorExt as _;

pub(crate) trait AsRawHandleExt: AsRawHandle {
	#[inline(always)]
	#[allow(clippy::as_conversions)]
	fn as_int_handle(&self) -> HANDLE {
		self.as_raw_handle() as HANDLE
	}
}
impl<T: AsRawHandle + ?Sized> AsRawHandleExt for T {}

pub(crate) trait HANDLEExt {
	fn to_std(self) -> RawHandle;
}
impl HANDLEExt for HANDLE {
	#[inline(always)]
	#[allow(clippy::as_conversions)]
	fn to_std(self) -> RawHandle {
		self as RawHandle
	}
}

pub(super) fn decode_eof<T>(r: io::Result<T>) -> io::Result<T> {
	use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
	match r {
		Err(e) if e.raw_os_error().eeq(ERROR_PIPE_NOT_CONNECTED) => {
			Err(io::Error::from(BrokenPipe))
		}
		els => els,
	}
}
pub(super) fn downgrade_eof<T: Default>(r: io::Result<T>) -> io::Result<T> {
	match decode_eof(r) {
		Err(e) if e.kind() == BrokenPipe => Ok(T::default()),
		els => els,
	}
}
