use super::*;
use crate::{os::windows::downgrade_eof, weaken_buf_init_mut};

impl RawPipeStream {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_to_uninit(weaken_buf_init_mut(buf))
    }
    fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.file_handle().read(buf)
    }
}

impl<Sm: PipeModeTag> PipeStream<pipe_mode::Bytes, Sm> {
    /// Same as `.read()` from the [`Read`] trait, but accepts an uninitialized buffer.
    #[inline]
    pub fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        downgrade_eof(self.raw.read_to_uninit(buf))
    }
}

impl<Sm: PipeModeTag> Read for &PipeStream<pipe_mode::Bytes, Sm> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        downgrade_eof(self.raw.read(buf))
    }
}
impl<Sm: PipeModeTag> Read for PipeStream<pipe_mode::Bytes, Sm> {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (self as &PipeStream<_, _>).read(buf)
    }
}
