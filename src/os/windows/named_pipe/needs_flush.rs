use super::{AtomicEnum, ReprU8};
use std::sync::atomic::Ordering::*;

#[derive(Debug)]
pub struct NeedsFlush(AtomicEnum<NeedsFlushVal>);
impl NeedsFlush {
    #[inline]
    pub fn mark_dirty(&self) {
        let _ = self.0.compare_exchange(
            NeedsFlushVal::No,
            NeedsFlushVal::Once,
            AcqRel,
            Relaxed, // We do not care about the loaded value
        );
    }
    #[inline]
    pub fn on_clone(&self) {
        self.0.store(NeedsFlushVal::Always, Release);
    }
    #[inline]
    pub fn on_flush(&self) -> bool {
        // TODO verify necessity of orderings
        match self.0.compare_exchange(NeedsFlushVal::Once, NeedsFlushVal::No, AcqRel, Acquire) {
            Ok(..) => true,
            Err(NeedsFlushVal::Always) => true,
            Err(.. /* NeedsFlushVal::No */) => false,
        }
    }
    #[inline]
    pub fn get(&mut self) -> bool {
        matches!(self.0.get_mut(), NeedsFlushVal::Once | NeedsFlushVal::Always)
    }
}
impl From<NeedsFlushVal> for NeedsFlush {
    #[inline]
    fn from(val: NeedsFlushVal) -> Self {
        Self(AtomicEnum::new(val))
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NeedsFlushVal {
    No,
    Once,
    Always,
}
unsafe impl ReprU8 for NeedsFlushVal {}
