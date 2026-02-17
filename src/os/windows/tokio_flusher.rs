use {
    crate::{
        os::windows::{c_wrappers, winprelude::*, NeedsFlush},
        UnpinExt, LOCK_POISON,
    },
    std::{
        future::{self, Future},
        io,
        mem::transmute,
        sync::{
            atomic::{AtomicBool, Ordering::*},
            Mutex,
        },
        task::{ready, Context, Poll},
        thread::Thread,
    },
    tokio::task::JoinHandle,
};

type FlushJH = JoinHandle<io::Result<()>>;

/// Wraps `FlushFileBuffers()` ran in a `spawn_blocking()` task into a poll interface.
#[derive(Debug)]
pub struct TokioFlusher {
    join_handle: Mutex<Option<FlushJH>>,
}
impl TokioFlusher {
    pub(crate) const fn new() -> Self { Self { join_handle: Mutex::new(None) } }
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
            // Idempotency optimization – don't flush unless there have been unflushed writes
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
        let fh = file_handle.as_int_handle();
        // Notifier prevents file handle UaF if execution of the spawned task
        // is delayed by a significant amount of time, as might happen if the
        // Tokio thread limit is hit (note that this UaF is benign from a
        // memory safety standpoint, since it's a handle UaF, but it may
        // produce undesirable behavior)
        // FIXME this is still somewhat fragile
        let notifier = Notifier::new();
        let notifier_ref = unsafe { transmute::<&Notifier, &'static Notifier>(&notifier) };
        let task = tokio::task::spawn_blocking(move || {
            notifier_ref.notify();
            c_wrappers::flush(unsafe { BorrowedHandle::borrow_raw(fh as _) })
        });
        let ret = join_handle.insert(task);
        notifier.wait();
        ret
    }
}
impl Default for TokioFlusher {
    #[inline]
    fn default() -> Self { Self::new() }
}

struct Notifier {
    thr: Thread,
    compl: AtomicBool,
    parked: AtomicBool,
}
impl Notifier {
    pub fn new() -> Self {
        Self {
            thr: std::thread::current(),
            compl: AtomicBool::new(false),
            parked: AtomicBool::new(false),
        }
    }
    pub fn wait(&self) {
        if !self.compl.load(Acquire) {
            self.parked.store(true, Release);
            loop {
                std::thread::park();
                if self.compl.load(Acquire) {
                    break;
                }
            }
        }
    }
    pub fn notify(&self) {
        self.compl.store(true, Release);
        if self.parked.load(Acquire) {
            self.thr.unpark();
        }
    }
}
