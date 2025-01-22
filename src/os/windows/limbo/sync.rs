use {
    crate::{
        os::windows::{
            limbo_pool::{LimboPool, MaybeReject},
            winprelude::*,
            FileHandle,
        },
        DebugExpectExt, OrErrno, LOCK_POISON,
    },
    std::{
        io,
        sync::{
            mpsc::{sync_channel, SyncSender, TrySendError},
            Mutex, OnceLock,
        },
        thread,
    },
    windows_sys::Win32::System::Pipes::DisconnectNamedPipe,
};

pub(crate) struct Corpse {
    pub handle: FileHandle,
    pub is_server: bool,
}
impl Corpse {
    #[inline]
    pub fn disconnect(&self) -> io::Result<()> {
        unsafe { DisconnectNamedPipe(self.handle.as_int_handle()).true_val_or_errno(()) }
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

fn limbo_keeper_name(idx: usize) -> String {
    match idx {
        usize::MAX => "limbo keeper".to_string(),
        x => format!("limbo keeper {}", x.wrapping_add(1)),
    }
}

pub(crate) fn send_off(c: Corpse) {
    fn bury(c: Corpse) { c.handle.flush().debug_expect("limbo flush failed"); }

    fn tryf(sender: &mut SyncSender<Corpse>, c: Corpse) -> MaybeReject<Corpse> {
        sender.try_send(c).map_err(|e| match e {
            TrySendError::Full(c) | TrySendError::Disconnected(c) => c,
        })
    }
    fn createf(idx: usize, c: Corpse) -> SyncSender<Corpse> {
        let (tx, rx) = sync_channel::<Corpse>(1);
        thread::Builder::new()
            .name(limbo_keeper_name(idx))
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
        thread::Builder::new()
            .name(limbo_keeper_name(idx))
            .spawn(move || {
                bury(c);
            })
            .debug_expect("failed to spawn newcomer to limbo pool");
    }

    let mutex = LIMBO.get_or_init(Default::default);
    let mut limbo = mutex.lock().expect(LOCK_POISON);

    limbo.linear_try_or_create(c, tryf, createf, fullf);
}
