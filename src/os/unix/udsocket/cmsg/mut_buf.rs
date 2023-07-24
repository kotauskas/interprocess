use super::{
    context::{Collector, DummyCollector},
    *,
};
use std::mem::MaybeUninit;

/// A mutable reference to a control message buffer that allows for insertion of ancillary data messages.
#[derive(Debug)]
pub struct CmsgMutBuf<'b, C = DummyCollector> {
    buf: &'b mut [MaybeUninit<u8>],
    init_len: usize,
    context_collector: C,
}
impl<'b> CmsgMutBuf<'b> {
    /// Creates a control message buffer from the given uninitialized slice.
    ///
    /// # Panics
    /// The buffer's length must not overflow `isize`.
    #[inline]
    pub fn new(buf: &'b mut [MaybeUninit<u8>]) -> Self {
        Self {
            buf,
            init_len: 0,
            context_collector: DummyCollector,
        }
    }
    /// Attaches a context collector to a `CmsgMutBuf` that doesn't have one.
    #[inline]
    pub fn add_collector<C>(self, context_collector: C) -> CmsgMutBuf<'b, C> {
        CmsgMutBuf {
            buf: self.buf,
            init_len: self.init_len,
            context_collector,
        }
    }
}
impl<'b> From<&'b mut [MaybeUninit<u8>]> for CmsgMutBuf<'b> {
    #[inline]
    fn from(buf: &'b mut [MaybeUninit<u8>]) -> Self {
        Self::new(buf)
    }
}
impl<'b, C> CmsgMutBuf<'b, C> {
    /// Creates a control message buffer from the given uninitialized slice and with the given context collector.
    #[inline]
    pub fn new_with_collector(buf: &'b mut [MaybeUninit<u8>], context_collector: C) -> Self {
        Self {
            buf,
            init_len: 0,
            context_collector,
        }
    }
    /// Transforms a `CmsgMutBuf` with one context collector type to a `CmsgMutBuf` with a different one via the given
    /// closure.
    #[inline]
    pub fn map_collector<C2>(self, f: impl FnOnce(C) -> C2) -> CmsgMutBuf<'b, C2> {
        CmsgMutBuf {
            buf: self.buf,
            init_len: self.init_len,
            context_collector: f(self.context_collector),
        }
    }
}

unsafe impl<C: Collector> CmsgMut for CmsgMutBuf<'_, C> {
    type Context = C;

    #[inline(always)]
    fn as_bytes(&self) -> &[MaybeUninit<u8>] {
        self.buf
    }
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.buf
    }
    #[inline(always)]
    fn valid_len(&self) -> usize {
        self.init_len
    }
    #[inline(always)]
    unsafe fn set_len(&mut self, new_len: usize) {
        self.init_len = new_len
    }
    #[inline(always)]
    fn context(&self) -> &Self::Context {
        &self.context_collector
    }
    #[inline(always)]
    fn context_mut(&mut self) -> &mut Self::Context {
        &mut self.context_collector
    }
}
