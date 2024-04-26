#![allow(dead_code)]

#[cfg(unix)]
use std::os::unix::io::RawFd;
use std::{
	io,
	mem::{transmute, MaybeUninit},
	num::Saturating,
	pin::Pin,
	sync::PoisonError,
};
#[cfg(windows)]
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};

/// Utility trait that, if used as a supertrait, prevents other crates from implementing the
/// trait.
pub(crate) trait Sealed {}
pub(crate) trait DebugExpectExt: Sized {
	fn debug_expect(self, msg: &str);
}

pub(crate) static LOCK_POISON: &str = "unexpected lock poison";
pub(crate) fn poison_error<T>(_: PoisonError<T>) -> io::Error {
	io::Error::other(LOCK_POISON)
}

pub(crate) trait OrErrno<T>: Sized {
	fn true_or_errno(self, f: impl FnOnce() -> T) -> io::Result<T>;
	#[inline(always)]
	fn true_val_or_errno(self, value: T) -> io::Result<T> {
		self.true_or_errno(|| value)
	}
	fn false_or_errno(self, f: impl FnOnce() -> T) -> io::Result<T>;
	#[inline(always)]
	fn false_val_or_errno(self, value: T) -> io::Result<T> {
		self.true_or_errno(|| value)
	}
}
impl<B: ToBool, T> OrErrno<T> for B {
	#[inline]
	fn true_or_errno(self, f: impl FnOnce() -> T) -> io::Result<T> {
		if self.to_bool() {
			Ok(f())
		} else {
			Err(io::Error::last_os_error())
		}
	}
	fn false_or_errno(self, f: impl FnOnce() -> T) -> io::Result<T> {
		if !self.to_bool() {
			Ok(f())
		} else {
			Err(io::Error::last_os_error())
		}
	}
}

// TODO(2.0.1) nonzero_or_errno

#[cfg(unix)]
pub(crate) trait FdOrErrno: Sized {
	fn fd_or_errno(self) -> io::Result<Self>;
}
#[cfg(unix)]
impl FdOrErrno for RawFd {
	#[inline]
	fn fd_or_errno(self) -> io::Result<Self> {
		(self != -1).true_val_or_errno(self)
	}
}

#[cfg(windows)]
pub(crate) trait HandleOrErrno: Sized {
	fn handle_or_errno(self) -> io::Result<Self>;
}
#[cfg(windows)]
impl HandleOrErrno for HANDLE {
	#[inline]
	fn handle_or_errno(self) -> io::Result<Self> {
		(self != INVALID_HANDLE_VALUE).true_val_or_errno(self)
	}
}

pub(crate) trait ToBool {
	fn to_bool(self) -> bool;
}
impl ToBool for bool {
	#[inline(always)]
	fn to_bool(self) -> bool {
		self
	}
}
impl ToBool for i32 {
	#[inline(always)]
	fn to_bool(self) -> bool {
		self != 0
	}
}

// TODO(2.0.1) add a helper for casting references to pointers and then forbid all as casts

impl<T, E: std::fmt::Debug> DebugExpectExt for Result<T, E> {
	#[inline]
	#[track_caller]
	fn debug_expect(self, msg: &str) {
		if cfg!(debug_assertions) {
			self.expect(msg);
		}
	}
}
impl<T> DebugExpectExt for Option<T> {
	#[inline]
	#[track_caller]
	fn debug_expect(self, msg: &str) {
		if cfg!(debug_assertions) {
			self.expect(msg);
		}
	}
}

pub(crate) trait NumExt: Sized {
	#[inline]
	fn saturate(self) -> Saturating<Self> {
		Saturating(self)
	}
}
impl<T> NumExt for T {}

#[inline(always)]
pub(crate) fn weaken_buf_init<T>(r: &[T]) -> &[MaybeUninit<T>] {
	unsafe {
		// SAFETY: same slice, weaker refinement
		transmute(r)
	}
}
#[inline(always)]
pub(crate) fn weaken_buf_init_mut<T>(r: &mut [T]) -> &mut [MaybeUninit<T>] {
	unsafe {
		// SAFETY: same here
		transmute(r)
	}
}

#[inline(always)]
pub(crate) unsafe fn assume_slice_init<T>(r: &[MaybeUninit<T>]) -> &[T] {
	unsafe {
		// SAFETY: same slice, stronger refinement
		transmute(r)
	}
}

pub(crate) trait UnpinExt: Unpin {
	#[inline]
	fn pin(&mut self) -> Pin<&mut Self> {
		Pin::new(self)
	}
}
impl<T: Unpin + ?Sized> UnpinExt for T {}
