use crate::{
    os::windows::{
        named_pipe::limbo_pool::{LimboPool, MaybeReject},
        winprelude::*,
        FileHandle,
    },
    DebugExpectExt,
};
use std::{
    io,
    sync::{
        mpsc::{sync_channel, SyncSender, TrySendError},
        Mutex, OnceLock,
    },
    thread,
};
use winapi::um::namedpipeapi::DisconnectNamedPipe;

pub(super) struct Corpse {
    pub handle: FileHandle,
    pub is_server: bool,
}
impl Corpse {
    #[inline]
    pub fn disconnect(&self) -> io::Result<()> {
        let success = unsafe { DisconnectNamedPipe(self.handle.0.as_raw_handle()) != 0 };
        ok_or_ret_errno!(success => ())
    }
}
impl Drop for Corpse {
    fn drop(&mut self) {
        if self.is_server {
            self.disconnect().debug_expect("named pipe server disconnect failed");
        }
    }
}

type Limbo = LimboPool<SyncSender<Corpse>>;
static LIMBO: OnceLock<Mutex<Limbo>> = OnceLock::new();

pub(super) fn send_off(c: Corpse) {
    fn bury(c: Corpse) {
        c.handle.flush().debug_expect("limbo flush failed");
    }

    fn tryf(sender: &mut SyncSender<Corpse>, c: Corpse) -> MaybeReject<Corpse> {
        sender.try_send(c).map_err(|e| match e {
            TrySendError::Full(c) | TrySendError::Disconnected(c) => c,
        })
    }
    fn createf(idx: usize, c: Corpse) -> SyncSender<Corpse> {
        let (tx, rx) = sync_channel::<Corpse>(1);
        thread::Builder::new()
            .name(format!("limbo keeper {}", idx + 1))
            .spawn(move || {
                while let Ok(h) = rx.recv() {
                    bury(h);
                }
            })
            .debug_expect("failed to spawn newcomer to limbo pool");
        tx.try_send(c).debug_expect("newcomer to limbo pool already failed");
        tx
    }
    fn fullf(idx: usize, c: Corpse) {
        let idx = idx.checked_add(1);
        let name = match idx {
            Some(idx) => format!("limbo keeper {}", idx + 1),
            None => "limbo keeper".to_string(),
        };
        thread::Builder::new()
            .name(name)
            .spawn(move || {
                bury(c);
            })
            .debug_expect("failed to spawn newcomer to limbo pool");
    }

    let mutex = LIMBO.get_or_init(Default::default);
    let mut limbo = mutex.lock().unwrap();

    limbo.linear_try_or_create(c, tryf, createf, fullf);
}
