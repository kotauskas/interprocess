use std::{
    fmt::{self, Debug, Formatter},
    sync::atomic::{AtomicU16, Ordering::*},
};
// TODO document needs-flush opt
pub struct NeedsFlush(AtomicU16);
impl NeedsFlush {
    #[inline]
    pub fn mark_dirty(&self) {
        let _ = self.0.compare_exchange(
            NeedsFlushVal::No.into(),
            NeedsFlushVal::Once.into(),
            AcqRel,
            Relaxed, // We do not care about the loaded value
        );
    }
    #[inline]
    pub fn on_clone(&self) {
        self.0.store(NeedsFlushVal::Always.into(), Release);
    }
    #[inline]
    pub fn on_flush(&self) -> bool {
        // TODO verify necessity of orderings
        match self
            .0
            .compare_exchange(NeedsFlushVal::Once.into(), NeedsFlushVal::No.into(), AcqRel, Acquire)
        {
            Ok(..) => true,
            Err(v) if v == NeedsFlushVal::Always.into() => true,
            Err(.. /* NeedsFlushVal::No */) => false,
        }
    }
    #[inline]
    pub fn get(&mut self) -> bool {
        matches!(
            (*self.0.get_mut()).try_into().unwrap(),
            NeedsFlushVal::Once | NeedsFlushVal::Always
        )
    }
}
impl From<NeedsFlushVal> for NeedsFlush {
    #[inline]
    fn from(value: NeedsFlushVal) -> Self {
        Self(AtomicU16::new(value.into()))
    }
}
impl Debug for NeedsFlush {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let val: NeedsFlushVal = self.0.load(SeqCst).try_into().unwrap();
        f.debug_tuple("NeedsFlush").field(&val).finish()
    }
}

// Specifically u16 is used because:
// - Probably marginally reduces false sharing
// - All platforms that support Windows support 16-bit atomics
// - The types `NeedsFlushVal` is contained in would have the second byte be padding otherwise
#[derive(Debug)]
#[repr(u16)]
pub enum NeedsFlushVal {
    No = 0,
    Once = 1,
    Always = 2,
}
impl From<NeedsFlushVal> for u16 {
    fn from(value: NeedsFlushVal) -> Self {
        value as _
    }
}
impl TryFrom<u16> for NeedsFlushVal {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::No),
            1 => Ok(Self::Once),
            2 => Ok(Self::Always),
            _ => Err(()),
        }
    }
}
