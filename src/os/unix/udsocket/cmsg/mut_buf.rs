use super::*;
use std::mem::MaybeUninit;

/// A mutable reference to a control message buffer that allows for insertion of ancillary data messages.
#[derive(Debug)]
pub struct CmsgMutBuf<'buf> {
    buf: &'buf mut [MaybeUninit<u8>],
    init_len: usize,
}
impl<'buf> CmsgMutBuf<'buf> {
    /// Creates a control message buffer from the given uninitialized slice.
    ///
    /// # Panics
    /// The buffer's length must not overflow `isize`.
    #[inline]
    pub fn new(buf: &'buf mut [MaybeUninit<u8>]) -> Self {
        Self { buf, init_len: 0 }
    }
}
impl<'buf> From<&'buf mut [MaybeUninit<u8>]> for CmsgMutBuf<'buf> {
    #[inline]
    fn from(buf: &'buf mut [MaybeUninit<u8>]) -> Self {
        Self::new(buf)
    }
}

unsafe impl CmsgMut for CmsgMutBuf<'_> {
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
}
