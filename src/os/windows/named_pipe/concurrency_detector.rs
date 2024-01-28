use std::sync::atomic::{AtomicBool, Ordering::*};

#[derive(Debug)]
pub struct ConcurrencyDetector(AtomicBool);
impl ConcurrencyDetector {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }
    #[track_caller]
    pub fn lock(&self) -> LockDetectorGuard<'_> {
        if self.0.compare_exchange(false, true, Acquire, Relaxed).is_err() {
            panic!(
                "\
concurrent I/O on a Windows named pipe attempted – this leads to deadlocks due to the underlying \
synchronization implemented by Windows"
            )
        }
        LockDetectorGuard(&self.0)
    }
}

pub struct LockDetectorGuard<'ld>(&'ld AtomicBool);
impl Drop for LockDetectorGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.store(false, Release)
    }
}
