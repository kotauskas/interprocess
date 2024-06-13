use crate::{
	os::windows::{winprelude::*, FileHandle, NeedsFlush},
	UnpinExt, LOCK_POISON,
};
use std::{
	future::{self, Future},
	io,
	sync::{atomic::Ordering::*, Mutex},
	task::{ready, Context, Poll},
};
use tokio::task::JoinHandle;

type FlushJH = JoinHandle<io::Result<()>>;

/// Wraps `FlushFileBuffers()` ran in a `spawn_blocking()` task into a poll interface.
#[derive(Debug)]
pub struct TokioFlusher {
	join_handle: Mutex<Option<FlushJH>>,
}
impl TokioFlusher {
	pub(crate) const fn new() -> Self {
		Self {
			join_handle: Mutex::new(None),
		}
	}
	#[inline]
	pub(crate) async fn flush_atomic(
		&self,
		file_handle: BorrowedHandle<'_>,
		needs_flush: &NeedsFlush,
	) -> io::Result<()> {
		future::poll_fn(|cx| self.poll_flush_atomic(file_handle, needs_flush, cx)).await
	}

	pub(crate) fn poll_flush_atomic(
		&self,
		file_handle: BorrowedHandle<'_>,
		needs_flush: &NeedsFlush,
		cx: &mut Context<'_>,
	) -> Poll<io::Result<()>> {
		if !needs_flush.get(Acquire) {
			// Idempotency optimization — don't flush unless there have been unflushed writes
			return Poll::Ready(Ok(()));
		}

		let mut flush = self.join_handle.lock().expect(LOCK_POISON);

		// The mutex is an acquire fence, so this load can safely be relaxed (it can actually be
		// non-atomic in practice, but there's hardly a performance benefit to that)
		if !needs_flush.get(Relaxed) {
			// Lock losering – don't flush if a different thread beat us to the lock
			return Poll::Ready(Ok(()));
		}

		let jh = Self::ensure_flush_start(&mut flush, file_handle);
		let rslt = ready!(jh.pin().poll(cx)).unwrap();
		if rslt.is_ok() {
			needs_flush.clear();
		}
		*flush = None;
		Poll::Ready(rslt)
	}

	pub(crate) fn poll_flush_mut(
		&self,
		file_handle: BorrowedHandle<'_>,
		needs_flush: &mut bool,
		cx: &mut Context<'_>,
	) -> Poll<io::Result<()>> {
		if !*needs_flush {
			// Idempotency optimization — don't flush unless there have been unflushed writes
			return Poll::Ready(Ok(()));
		}

		let mut flush = self.join_handle.lock().expect(LOCK_POISON);

		let jh = Self::ensure_flush_start(&mut flush, file_handle);
		let rslt = ready!(jh.pin().poll(cx)).unwrap();
		if rslt.is_ok() {
			*needs_flush = false;
		}
		*flush = None;
		Poll::Ready(rslt)
	}

	fn ensure_flush_start<'opt>(
		join_handle: &'opt mut Option<FlushJH>,
		file_handle: BorrowedHandle<'_>,
	) -> &'opt mut FlushJH {
		if let Some(jh) = join_handle {
			return jh;
		}
		let handle = file_handle.as_int_handle();
		let task = tokio::task::spawn_blocking(move || FileHandle::flush_hndl(handle));
		join_handle.insert(task)
	}
}
impl Default for TokioFlusher {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}
