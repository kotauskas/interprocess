use crate::AsMutPtr;
use std::{mem::ManuallyDrop, ops::Deref, ptr, sync::Arc};

/// Inlining optimization for `Arc`.
#[derive(Debug)]
pub enum MaybeArc<T> {
	Inline(T),
	Shared(Arc<T>),
}
impl<T> MaybeArc<T> {
	/// `Arc::clone` in place.
	// TODO(2.3.0) this whole function is dodgy, Miri correction needed
	pub fn refclone(&mut self) -> Self {
		let arc = match self {
			Self::Inline(mx) => {
				let x = unsafe {
					// SAFETY: generally a no-op from a safety perspective; the ManuallyDrop ensures
					// that it stays that way in the event of a panic in Arc::new
					ptr::read(mx.as_mut_ptr().cast::<ManuallyDrop<T>>())
				};
				let arc = Arc::new(x);

				// BEGIN no-panic zone
				let arc = unsafe {
					// SAFETY: ManuallyDrop is layout-transparent
					Arc::from_raw(Arc::into_raw(arc).cast::<T>())
				};
				unsafe {
					// SAFETY: self, being a mutable reference, is valid for writes
					ptr::write(self, Self::Shared(arc));
				}
				// END no-panic zone, the danger has passed

				let ref_for_clone = match self {
					Self::Shared(s) => &*s,
					Self::Inline(..) => unreachable!(),
				};
				Arc::clone(ref_for_clone)
			}
			Self::Shared(arc) => Arc::clone(arc),
		};
		Self::Shared(arc)
	}
	pub fn try_make_owned(&mut self) -> bool {
		if let Self::Shared(am) = self {
			let a = unsafe { ptr::read(am) };
			if let Ok(x) = Arc::try_unwrap(a) {
				unsafe {
					ptr::write(self, Self::Inline(x));
				}
				true
			} else {
				false
			}
		} else {
			true
		}
	}
	pub fn ptr_eq(this: &Self, other: &Self) -> bool {
		match this {
			Self::Inline(..) => false,
			Self::Shared(a) => match other {
				Self::Inline(..) => false,
				Self::Shared(b) => Arc::ptr_eq(a, b),
			},
		}
	}
}
impl<T> Deref for MaybeArc<T> {
	type Target = T;
	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		match self {
			Self::Inline(x) => x,
			Self::Shared(a) => a,
		}
	}
}
impl<T> From<T> for MaybeArc<T> {
	#[inline]
	fn from(x: T) -> Self {
		Self::Inline(x)
	}
}
impl<T> From<Arc<T>> for MaybeArc<T> {
	#[inline]
	fn from(a: Arc<T>) -> Self {
		Self::Shared(a)
	}
}
