use super::*;
use std::{collections::TryReserveError, mem::MaybeUninit, slice};

/// A **c**ontrol **m**e**s**sa**g**e buffer, used to store the encoded form of ancillary data.
#[derive(Clone, Debug, Default)]
pub struct CmsgVecBuf {
    buf: Vec<u8>,
    trunc: bool,
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
    pub fn from_buf(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self { buf, trunc: false }
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer will lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        Self { buf, trunc: false }
    }
}

unsafe impl CmsgMut for CmsgVecBuf {
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
    #[inline]
    fn reserve(&mut self, additional: usize) -> ReserveResult {
        self.buf.try_reserve(additional).map_err(mkerr)
    }
    #[inline]
    fn reserve_exact(&mut self, additional: usize) -> ReserveResult {
        self.buf.try_reserve_exact(additional).map_err(mkerr)
    }
    fn is_truncated(&self) -> bool {
        self.trunc
    }
    fn set_truncation_flag(&mut self, flag: bool) {
        self.trunc = flag;
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
