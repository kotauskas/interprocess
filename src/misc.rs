use std::{
    mem::{transmute, MaybeUninit},
    pin::Pin,
};

/// A utility trait that, if used as a supertrait, prevents other crates from implementing the
/// trait. If the trait itself was pub(crate), it wouldn't work as a supertrait on public traits. We
/// use a private module instead to make it impossible to name the trait from outside the crate.
pub trait Sealed {}
pub(crate) trait DebugExpectExt: Sized {
    fn debug_expect(self, msg: &str);
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

#[inline(always)]
#[allow(dead_code)]
pub(crate) fn weaken_buf_init<T>(r: &[T]) -> &[MaybeUninit<T>] {
    unsafe {
        // SAFETY: same slice, weaker refinement
        transmute(r)
    }
}
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn weaken_buf_init_mut<T>(r: &mut [T]) -> &mut [MaybeUninit<T>] {
    unsafe {
        // SAFETY: same here
        transmute(r)
    }
}

#[inline(always)]
#[allow(dead_code)]
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
