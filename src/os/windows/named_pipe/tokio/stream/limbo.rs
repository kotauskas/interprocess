//! Does not use the limbo pool.

use super::*;
use crate::{
	os::windows::{winprelude::*, FileHandle},
	DebugExpectExt, LOCK_POISON,
};
use std::sync::{Mutex, OnceLock};
use tokio::{
	runtime::{self, Handle as RuntimeHandle, Runtime},
	sync::mpsc::{unbounded_channel, UnboundedSender},
	task,
};

pub(super) struct Corpse(pub InnerTokio);
impl Drop for Corpse {
	fn drop(&mut self) {
		if let InnerTokio::Server(server) = &self.0 {
			server
				.disconnect()
				.debug_expect("named pipe server disconnect failed");
		}
	}
}

type Limbo = UnboundedSender<Corpse>;
static LIMBO: OnceLock<Mutex<Limbo>> = OnceLock::new();
static LIMBO_RT: OnceLock<Runtime> = OnceLock::new();

fn static_runtime_handle() -> &'static RuntimeHandle {
	LIMBO_RT
		.get_or_init(|| {
			runtime::Builder::new_multi_thread()
				.worker_threads(1)
				.enable_io()
				.thread_name("Tokio limbo dispatcher")
				.thread_stack_size(1024 * 1024)
				.build()
				.expect(
					"\
failed to build Tokio limbo helper (only necessary if the first named pipe to be dropped happens \
to go out of scope outside of another Tokio runtime)",
				)
		})
		.handle()
}

fn bury(c: Corpse) {
	task::spawn_blocking(move || {
		let handle = c.0.as_int_handle();
		FileHandle::flush_hndl(handle).debug_expect("limbo flush failed");
	});
}

fn create_limbo() -> Limbo {
	let (tx, mut rx) = unbounded_channel();

	let mut _guard = None;
	if RuntimeHandle::try_current().is_err() {
		_guard = Some(static_runtime_handle().enter());
	}
	task::spawn(async move {
		while let Some(c) = rx.recv().await {
			bury(c);
		}
	});

	tx
}

pub(super) fn send_off(c: Corpse) {
	let mutex = LIMBO.get_or_init(|| Mutex::new(create_limbo()));
	let mut limbo = mutex.lock().expect(LOCK_POISON);
	if let Err(c) = limbo.send(c) {
		*limbo = create_limbo();
		limbo
			.send(c.0)
			.ok()
			.debug_expect("fresh Tokio limbo helper died immediately after being created");
	}
}
