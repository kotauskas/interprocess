use super::{
    context::{Collector, DummyCollector},
    *,
};
use std::{collections::TryReserveError, mem::MaybeUninit, slice};

/// A **c**ontrol **m**e**s**sa**g**e buffer, used to store the encoded form of ancillary data.
pub struct CmsgVecBuf<C = DummyCollector> {
    buf: Vec<u8>,
    /// The context collector stored alongside the buffer.
    ///
    /// `.as_ref()` and `.as_mut()` borrow this field (immutably and mutably, respectively) for decoding and context
    /// collection respectively.
    pub context_collector: C,
}
impl CmsgVecBuf {
    /// Creates a buffer with the specified capacity. Using a capacity of 0 makes for a useless buffer, but does not
    /// allocate.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self::from_buf(Vec::with_capacity(capacity))
    }
    /// Converts a `Vec<u8>` to a `CmsgBuffer`, discarding all its data in the process.
    #[inline]
    pub fn from_buf(buf: Vec<u8>) -> Self {
        Self::from_buf_with_collector(buf, DummyCollector)
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer will lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        Self {
            buf,
            context_collector: DummyCollector,
        }
    }
    #[inline]
    /// Attaches a context collector to a `CmsgVecBuf` that doesn't have one.
    pub fn add_collector<C>(self, context_collector: C) -> CmsgVecBuf<C> {
        CmsgVecBuf {
            buf: self.buf,
            context_collector,
        }
    }
}
impl<C: Collector> CmsgVecBuf<C> {
    /// Creates a buffer with the specified capacity and an owned context collector. Using a capacity of 0 makes for a
    /// useless buffer, but does not allocate.
    #[inline]
    pub fn new_with_collector(capacity: usize, context_collector: C) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            context_collector,
        }
    }
    /// Converts a `Vec<u8>` to a `CmsgBuffer`, discarding all its data in the process. The given context collector is
    /// also added into the mix.
    pub fn from_buf_with_collector(mut buf: Vec<u8>, context_collector: C) -> Self {
        buf.clear();
        Self { buf, context_collector }
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    /// The given context collector is also added into the mix.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer may lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_with_collector_unchecked(buf: Vec<u8>, context_collector: C) -> Self {
        Self { buf, context_collector }
    }
    /// Transforms a `CmsgVecBuf` with one context collector type to a `CmsgVecBuf` with a different one via the given
    /// closure.
    #[inline]
    pub fn map_collector<C2>(self, f: impl FnOnce(C) -> C2) -> CmsgVecBuf<C2> {
        CmsgVecBuf {
            buf: self.buf,
            context_collector: f(self.context_collector),
        }
    }
}
unsafe impl<C: Collector> CmsgMut for CmsgVecBuf<C> {
    type Context = C;
    #[inline(always)]
    fn as_bytes(&self) -> &[MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts(self.buf.as_ptr().cast::<MaybeUninit<u8>>(), self.buf.capacity()) }
    }
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts_mut(self.buf.as_mut_ptr().cast::<MaybeUninit<u8>>(), self.buf.capacity()) }
    }
    #[inline(always)]
    fn valid_len(&self) -> usize {
        self.buf.len()
    }
    #[inline(always)]
    unsafe fn set_len(&mut self, new_len: usize) {
        unsafe { self.buf.set_len(new_len) }
    }
    #[inline(always)]
    fn context(&self) -> &Self::Context {
        &self.context_collector
    }
    #[inline(always)]
    fn context_mut(&mut self) -> &mut Self::Context {
        &mut self.context_collector
    }
    #[inline]
    fn reserve(&mut self, additional: usize) -> ReserveResult {
        self.buf.try_reserve(additional).map_err(mkerr)
    }
    #[inline]
    fn reserve_exact(&mut self, additional: usize) -> ReserveResult {
        self.buf.try_reserve_exact(additional).map_err(mkerr)
    }
}
impl From<Vec<u8>> for CmsgVecBuf {
    #[inline]
    fn from(buf: Vec<u8>) -> Self {
        Self::from_buf(buf)
    }
}

fn mkerr(e: TryReserveError) -> ReserveError {
    ReserveError::Failed(Box::new(e))
}
