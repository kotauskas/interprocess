#![allow(dead_code)]

use std::{
	io,
	mem::{transmute, MaybeUninit},
	num::Saturating,
	pin::Pin,
	sync::PoisonError,
};

/// Utility trait that, if used as a supertrait, prevents other crates from implementing the
/// trait.
pub(crate) trait Sealed {}
pub(crate) trait DebugExpectExt: Sized {
	fn debug_expect(self, msg: &str);
}

pub static LOCK_POISON: &str = "unexpected lock poison";
pub fn poison_error<T>(_: PoisonError<T>) -> io::Error {
	io::Error::other(LOCK_POISON)
}

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
