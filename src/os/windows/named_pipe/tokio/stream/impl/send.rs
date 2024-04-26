use super::*;
use crate::{
	os::windows::{named_pipe::PmtNotNone, winprelude::*, FileHandle},
	UnpinExt, LOCK_POISON,
};
use std::sync::MutexGuard;
use tokio::io::AsyncWrite;

impl RawPipeStream {
	fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
		loop {
			ready!(same_clsrv!(x in self.inner() => x.poll_write_ready(cx)))?;
			match same_clsrv!(x in self.inner() => x.try_write(buf)) {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
				els => {
					self.needs_flush.mark_dirty();
					return Poll::Ready(els);
				}
			}
		}
	}
}

impl<Rm: PipeModeTag, Sm: PipeModeTag + PmtNotNone> PipeStream<Rm, Sm> {
	fn ensure_flush_start(&self, slf_flush: &mut MutexGuard<'_, Option<FlushJH>>) {
		if slf_flush.is_some() {
			return;
		}

		let handle = self.as_int_handle();
		let task = tokio::task::spawn_blocking(move || FileHandle::flush_hndl(handle));

		**slf_flush = Some(task);
	}
	/// Flushes the stream, waiting until the send buffer is empty (has been received by the other
	/// end in its entirety).
	///
	/// Only available on streams that have a send mode.
	#[inline]
	pub async fn flush(&self) -> io::Result<()> {
		future::poll_fn(|cx| self.poll_flush(cx)).await
	}

	/// Polls the future of `.flush()`.
	pub fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		if !self.raw.needs_flush.on_flush() {
			// No flush required.
			return Poll::Ready(Ok(()));
		}

		let mut flush = self.flush.lock().expect(LOCK_POISON);
		let rslt = loop {
			match flush.as_mut() {
				Some(fl) => break ready!(fl.pin().poll(cx)).unwrap(),
				None => self.ensure_flush_start(&mut flush),
			}
		};
		*flush = None;
		if rslt.is_err() {
			self.raw.needs_flush.mark_dirty();
		}
		Poll::Ready(rslt)
	}

	/// Marks the stream as unflushed, preventing elision of the next flush operation (which
	/// includes limbo).
	#[inline]
	pub fn mark_dirty(&self) {
		self.raw.needs_flush.mark_dirty();
	}
	/// Assumes that the other side has consumed everything that's been written so far. This will
	/// turn the next flush into a no-op, but will cause the send buffer to be cleared when the
	/// stream is closed, since it won't be sent to limbo.
	///
	/// If there's already an outstanding `.flush()` operation, it won't be affected by this call.
	#[inline]
	pub fn assume_flushed(&self) {
		self.raw.needs_flush.on_flush();
	}
	/// Drops the stream without sending it to limbo. This is the same as calling `assume_flushed()`
	/// right before dropping it.
	///
	/// If there's already an outstanding `.flush()` operation, it won't be affected by this call.
	#[inline]
	pub fn evade_limbo(self) {
		self.assume_flushed();
	}
}

impl<Rm: PipeModeTag> PipeStream<Rm, pipe_mode::Messages> {
	/// Sends a message into the pipe, returning how many bytes were successfully sent (typically
	/// equal to the size of what was requested to be sent).
	#[inline]
	pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
		struct Write<'a>(&'a RawPipeStream, &'a [u8]);
		impl Future for Write<'_> {
			type Output = io::Result<usize>;
			#[inline]
			fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
				let slf = self.get_mut();
				slf.0.poll_write(cx, slf.1)
			}
		}
		Write(&self.raw, buf).await
	}
}

impl<Rm: PipeModeTag> AsyncWrite for &PipeStream<Rm, pipe_mode::Bytes> {
	#[inline(always)]
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<Result<usize, io::Error>> {
		self.get_mut().raw.poll_write(cx, buf)
	}
	#[inline(always)]
	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		self.get_mut().poll_flush(cx)
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		// TODO(2.0.0) actually close connection here
		AsyncWrite::poll_flush(self, cx)
	}
}
impl<Rm: PipeModeTag> AsyncWrite for PipeStream<Rm, pipe_mode::Bytes> {
	#[inline]
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<Result<usize, io::Error>> {
		AsyncWrite::poll_write((&mut &*self).pin(), cx, buf)
	}
	#[inline]
	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		AsyncWrite::poll_flush((&mut &*self).pin(), cx)
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		AsyncWrite::poll_shutdown((&mut &*self).pin(), cx)
	}
}
