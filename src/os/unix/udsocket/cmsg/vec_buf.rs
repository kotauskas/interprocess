use super::*;
use std::{collections::TryReserveError, mem::MaybeUninit, slice};

/// A **c**ontrol **m**e**s**sa**g**e buffer, used to store the encoded form of ancillary data.
#[derive(Clone, Debug, Default)]
pub struct CmsgVecBuf(Vec<u8>);
impl CmsgVecBuf {
    /// Creates a buffer with the specified capacity. Using a capacity of 0 makes for a useless buffer, but does not
    /// allocate.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    /// Converts a `Vec<u8>` to a `CmsgBuffer`, discarding all its data in the process.
    #[inline]
    pub fn from_buf(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self(buf)
    }
    /// Creates a control message buffer without clearing it first. The contents are assumed to be valid ancillary data.
    ///
    /// # Safety
    /// Having arbitrary data in the buffer will lead to invalid memory accesses inside the system C library.
    #[inline]
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        Self(buf)
    }
}

unsafe impl CmsgMut for CmsgVecBuf {
    #[inline(always)]
    fn as_bytes(&self) -> &[MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts(self.0.as_ptr().cast::<MaybeUninit<u8>>(), self.0.capacity()) }
    }
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts_mut(self.0.as_mut_ptr().cast::<MaybeUninit<u8>>(), self.0.capacity()) }
    }
    #[inline(always)]
    fn valid_len(&self) -> usize {
        self.0.len()
    }
    #[inline(always)]
    unsafe fn set_len(&mut self, new_len: usize) {
        unsafe { self.0.set_len(new_len) }
    }
    #[inline]
    fn reserve(&mut self, additional: usize) -> ReserveResult {
        self.0.try_reserve(additional).map_err(mkerr)
    }
    #[inline]
    fn reserve_exact(&mut self, additional: usize) -> ReserveResult {
        self.0.try_reserve_exact(additional).map_err(mkerr)
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
