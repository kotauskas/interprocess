use std::{mem::ManuallyDrop, ops::Deref, ptr, sync::Arc};

/// Inlining optimization for `Arc`.
#[derive(Debug)]
pub enum MaybeArc<T> {
    Inline(T),
    Shared(Arc<T>),
}
impl<T> MaybeArc<T> {
    /// `Arc::clone` in place.
    pub fn refclone(&mut self) -> Self {
        let arc = match self {
            Self::Inline(mx) => {
                let x = unsafe {
                    // SAFETY: generally a no-op from a safety perspective; the ManuallyDrop ensures
                    // that it stays that way in the event of a panic
                    ptr::read((mx as *mut T).cast::<ManuallyDrop<T>>())
                };
                // Nothing can panic past the following line
                let arc = Arc::new(x);
                let arc = unsafe {
                    // SAFETY: ManuallyDrop is layout-transparent
                    Arc::from_raw(Arc::into_raw(arc).cast::<T>())
                };
                let clone = Arc::clone(&arc);
                *self = Self::Shared(arc);
                clone
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
