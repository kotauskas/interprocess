use std::{
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering::*},
};

pub struct ConcurrencyDetector<S>(AtomicBool, PhantomData<S>);
impl<S: ConcurrencyDetectionSite> ConcurrencyDetector<S> {
    pub const fn new() -> Self { Self(AtomicBool::new(false), PhantomData) }
    #[track_caller]
    #[must_use]
    pub fn lock(&self) -> LockDetectorGuard<'_> {
        if self.0.compare_exchange(false, true, Acquire, Relaxed).is_err() {
            concurrency_detected(S::NAME, S::WOULD_ACTUALLY_DEADLOCK);
        }
        LockDetectorGuard(&self.0)
    }
}
#[cold]
#[track_caller]
fn concurrency_detected(primname: &str, deadlock: bool) -> ! {
    let reason = if deadlock {
        "because it would have caused a deadlock"
    } else {
        "to avoid portability issues"
    };
    panic!(
        "\
concurrent I/O with a {primname} attempted – this leads to deadlocks due to the synchronization \
used by named pipes on Windows internally, and was prevented {reason}",
    )
}
impl<M: ConcurrencyDetectionSite> Debug for ConcurrencyDetector<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcurrencyDetector")
            .field("locked", &self.0)
            .field("primname", &M::NAME)
            .field("would_actually_deadlock", &M::WOULD_ACTUALLY_DEADLOCK)
            .finish()
    }
}

pub trait ConcurrencyDetectionSite {
    const NAME: &'static str;
    const WOULD_ACTUALLY_DEADLOCK: bool;
}

#[derive(Default)]
pub struct LocalSocketSite;
impl ConcurrencyDetectionSite for LocalSocketSite {
    const NAME: &'static str = "local socket";
    // Concurrency detection for named pipes happens within named pipes.
    const WOULD_ACTUALLY_DEADLOCK: bool = false;
}

pub struct LockDetectorGuard<'ld>(&'ld AtomicBool);
impl Drop for LockDetectorGuard<'_> {
    #[inline]
    fn drop(&mut self) { self.0.store(false, Release) }
}
