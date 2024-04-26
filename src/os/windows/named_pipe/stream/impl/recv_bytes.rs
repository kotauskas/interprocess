use super::*;
use crate::{os::windows::downgrade_eof, weaken_buf_init_mut};

impl RawPipeStream {
	#[track_caller]
	fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
		self.read_to_uninit(weaken_buf_init_mut(buf))
	}
	#[track_caller]
	fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
		let _guard = self.concurrency_detector.lock();
		self.file_handle().read(buf)
	}
}

impl<Sm: PipeModeTag> PipeStream<pipe_mode::Bytes, Sm> {
	/// Same as `.read()` from the [`Read`] trait, but accepts an uninitialized buffer.
	///
	/// Interacts with [concurrency prevention](#concurrency-prevention).
	#[inline]
	pub fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
		downgrade_eof(self.raw.read_to_uninit(buf))
	}
}

/// Interacts with [concurrency prevention](#concurrency-prevention).
impl<Sm: PipeModeTag> Read for &PipeStream<pipe_mode::Bytes, Sm> {
	#[inline]
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		downgrade_eof(self.raw.read(buf))
	}
}
/// Interacts with [concurrency prevention](#concurrency-prevention).
impl<Sm: PipeModeTag> Read for PipeStream<pipe_mode::Bytes, Sm> {
	#[inline(always)]
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		(&*self).read(buf)
	}
}
