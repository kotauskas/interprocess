//! Does not use the limbo pool.

use std::ops::Deref;
use crate::{
	os::windows::{winprelude::*, FileHandle},
	DebugExpectExt, LOCK_POISON,
};
use std::sync::{Mutex, OnceLock};
use tokio::{fs::File, net::windows::named_pipe::{NamedPipeClient, NamedPipeServer}, runtime::{Handle as RuntimeHandle}, sync::mpsc::{unbounded_channel, UnboundedSender}, task};
use tokio::runtime::{Builder, Runtime};

pub(crate) enum Corpse {
	NpServer(NamedPipeServer),
	NpClient(NamedPipeClient),
	Unnamed(File),
}
impl Drop for Corpse {
	fn drop(&mut self) {
		if let Self::NpServer(server) = self {
			server
				.disconnect()
				.debug_expect("named pipe server disconnect failed");
		}
	}
}
impl AsRawHandle for Corpse {
	fn as_raw_handle(&self) -> RawHandle {
		match self {
			Corpse::NpServer(o) => o.as_raw_handle(),
			Corpse::NpClient(o) => o.as_raw_handle(),
			Corpse::Unnamed(o) => o.as_raw_handle(),
		}
	}
}

type Limbo = UnboundedSender<Corpse>;
static LIMBO: Mutex<Option<Limbo>> = Mutex::new(None);

fn bury(c: Corpse) {
	task::spawn_blocking(move || {
		let handle = c.as_int_handle();
		FileHandle::flush_hndl(handle).debug_expect("limbo flush failed");
	});
}

fn create_limbo() -> Option<Limbo> {
	if RuntimeHandle::try_current().is_err() {
		return None;
	}

	let (tx, mut rx) = unbounded_channel();
	task::spawn(async move {
		while let Some(c) = rx.recv().await {
			bury(c);
		}
	});
	
	if tx.is_closed() {
		// The tokio runtime may still have a handle, but we're right in the process of the runtime shutdown. 
		// When tokio is shutting down, it will drop tasks directly and synchronously at task::spawn methods.
		// tx.is_closed() will evaluate to true in that case, because the channel receiver is dropped along with the task.
		None
	} else {
		Some(tx)
	}
}

pub(crate) fn send_off(c: Corpse) {
	if let Some(limbo) = GUARANTEED_LIMBO.get() {
		limbo.send(c).debug_expect("Guaranteed limbo must always be available");
		return;
	}
	
	let mut limbo_guard = LIMBO.lock().expect(LOCK_POISON);
	let limbo = match limbo_guard.as_ref() {
		Some(limbo) => Some(limbo),
		// if no limbo exists, create one
		None => {
			*limbo_guard = create_limbo();
			limbo_guard.as_ref()
		}
	};
	
	let Some(limbo) = limbo else {
		// no user tokio runtime available for limbo, sending to guaranteed limbo
		drop(limbo_guard);
		send_off_to_guaranteed_limbo(c);
		return;
	};
	
	// try to send the corpse to the limbo
	let c = match limbo.send(c) {
		Ok(_) => return,
		Err(c) => c.0,
	};

	// we lost the limbo, but maybe it ran on a different tokio runtime which has died in the meantime
	// try again using a fresh limbo on the current tokio runtime
	
	*limbo_guard = create_limbo();
	let Some(limbo) = limbo_guard.as_ref() else {
		// no user tokio runtime available for limbo, sending to guaranteed limbo
		drop(limbo_guard);
		send_off_to_guaranteed_limbo(c);
		return;
	};

	let c = match limbo.send(c) {
		Ok(_) => return,
		Err(c) => c.0,
	};
	
	// we lost the limbo again, now we have no other option than to send to the guaranteed limbo
	*limbo_guard = None;
	drop(limbo_guard);
	send_off_to_guaranteed_limbo(c);
}


// the guaranteed limbo is running on its own tokio runtime.
// it is initialized as a last resort if no other tokio runtime is available.
struct GuaranteedLimbo {
	runtime: Runtime,
	limbo: Limbo
}

impl Deref for GuaranteedLimbo {
	type Target = Limbo;
	fn deref(&self) -> &Self::Target {
		&self.limbo
	}
}

static GUARANTEED_LIMBO: OnceLock<GuaranteedLimbo> = OnceLock::new();

fn send_off_to_guaranteed_limbo(c: Corpse) {
	let limbo = GUARANTEED_LIMBO.get_or_init(|| {
		let (tx, mut rx) = unbounded_channel();

		let runtime = Builder::new_multi_thread()
			.worker_threads(1)
			.enable_io()
			.thread_name("Tokio limbo dispatcher")
			.thread_stack_size(1024 * 1024)
			.build()
			.expect(
				"\
failed to build Tokio limbo helper (only necessary if the first pipe to be dropped happens to go \
out of scope outside of another Tokio runtime)",
			);

		runtime.spawn(async move {
			while let Some(c) = rx.recv().await {
				bury(c);
			}
		});

		GuaranteedLimbo { runtime, limbo: tx }
	});

	limbo.send(c).debug_expect("Guaranteed limbo must always be available");
}
