use std::{
    fmt::{self, Debug, Formatter},
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc, RwLock,
    },
};

pub struct Instancer<T>(pub RwLock<Vec<Instance<T>>>);
impl<T> Instancer<T> {
    pub fn allocate(&self) -> Option<Instance<T>> {
        let instances = self.0.read().expect("unexpected lock poison");

        // Finds the first unoccupied instance, returning `Some` if one is found or `None` if all
        // are busy.
        instances.iter().filter_map(Instance::try_take).next()
    }
    pub fn add_instance(&self, instance: T) -> Instance<T> {
        let [inst, inst_c] = Instance::create_taken(instance);
        let mut instances = self.0.write().expect("unexpected lock poison");
        instances.push(inst);
        inst_c
    }
}

impl<T: Debug> Debug for Instancer<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let instances = self.0.read().expect("unexpected lock poison");
        f.debug_tuple("Instancer").field(instances.deref()).finish()
    }
}

/// Utility struct that implements atomic ownership transfer and splitting in the instancer.
#[repr(transparent)]
pub struct Instance<T>(Arc<InstanceInner<T>>);
struct InstanceInner<T> {
    instance: T,
    split: AtomicBool,
    out_of_instancer: AtomicBool,
}
impl<T> InstanceInner<T> {
    pub fn new(instance: T, taken: bool) -> Self {
        Self {
            instance,
            split: AtomicBool::new(false),
            out_of_instancer: AtomicBool::new(taken),
        }
    }
}
impl<T> Instance<T> {
    pub fn create_taken(instance: T) -> [Self; 2] {
        let i = Self::new(instance, true);
        let b = Arc::clone(&i.0);
        [i, Self(b)]
    }
    pub fn create_non_taken(instance: T) -> Self {
        Self::new(instance, false)
    }
    pub fn new(instance: T, taken: bool) -> Self {
        let ii = InstanceInner::new(instance, taken);
        Self(Arc::new(ii))
    }
    pub fn instance(&self) -> &T {
        &self.0.deref().instance
    }
    pub fn is_server(&self) -> bool {
        // When a listener lends an instance, it sets the flag and then clears it when the instance
        // is released (meaning that it's set for the whole lifetime of a connection), while client
        // connections initialize it cleared and never touch it at all.
        //
        // This can be `Relaxed`. In the case of non-split instances, the taken instance is either
        // on the same thread and thus doesn't need synchronization, or it's on a different thread
        // and all of the relevant synchronization is performed as part of sending it to another
        // thread (learned this from comments in the standard library's `impl Clone for Arc<T>`).
        //
        // In the case of split instances, the `false` store of a split server-side instance can
        // only happen in the drop code of the split half that gets dropped later than the other,
        // and by that time, the `.is_server()` method is inaccessible to both halves.
        self.0.out_of_instancer.load(Relaxed)
    }
    pub fn is_split(&self) -> bool {
        // This can be `Relaxed`, because the other split half is either on the same thread and thus
        // doesn't need synchronization to read the current value here, or it's on a different
        // thread and all of the relevant synchronization is performed as part of sending it to
        // another thread (same reasoning as above).
        self.0.split.load(Relaxed)
    }
    pub fn try_take(&self) -> Option<Self> {
        if self
            .0
            .out_of_instancer
            // We don't want spurious fails, because we'd have to give up the whole instance and
            // look for the next one. If this was the last one, allocating a new instance is a cost
            // that's probably not amortized by the savings of using `.compare_exchange_weak()`.
            .compare_exchange(false, true, AcqRel, Relaxed)
            .is_ok()
        {
            let refclone = Arc::clone(&self.0);
            Some(Self(refclone))
        } else {
            None
        }
    }
    pub fn split(&self) -> Self {
        // This can be a relaxed load because a non-split instance won't ever be shared between
        // threads. From a correctness standpoint, this could even be a non-atomic load, but because
        // most architectures already guarantee well-aligned memory accesses to be atomic, there's
        // no point to writing unsafe code to do that. (Also, this condition obviously signifies
        // a bug in interprocess that can only lead to creation of excess instances at worst, so
        // there isn't a real point to making sure it never happens in release mode.)
        debug_assert!(
            !self.0.split.load(Relaxed),
            "cannot split an already split instance"
        );
        // Again, the store doesn't even need to be atomic because it won't happen concurrently.
        self.0.split.store(true, Relaxed);

        let refclone = Arc::clone(&self.0);
        Self(refclone)
    }
}

impl<T> Drop for Instance<T> {
    fn drop(&mut self) {
        // First, we try to declare that the instance is no longer split, if it even was split to
        // begin with.
        if self
            .0
            .split
            .compare_exchange(true, false, AcqRel, Relaxed)
            .is_err()
        {
            // If that failed, it can only mean that the instance was not split by that point. This
            // means that either this current shared reference is the only one to own the instance
            // outside of the instancer, or it's the instancer that's being dropped. In the former
            // case, we must tell the instancer that the instance is free to use again, but in the
            // latter one, it would just be a store of the same value that will never be read again.
            //
            // To avoid that store, we'd have to figure out whether we're outside or inside of the
            // instancer, which would incur an additional load. A load done solely to avoid a store
            // to the same place is almost certainly a waste of time for an atomic variable that's
            // not contended between threads, i.e. a store won't be substantially more expensive
            // than a load in the absence of ongoing cache line bouncing, which can't possibly be a
            // thing if we're dropping the instancer.
            //
            // In other words, because asking the instancer for new instances and dropping it at the
            // same time is not a thing that happens, an unconditional store would most certainly be
            // cheaper than a load-guarded store, from what I can tell. That being said, if you have
            // so much time to waste that you actually want to benchmark this and see if avoiding
            // the store when possible actually improves performance of system-call heavy code, an
            // absurd supposition to anyone who's not interested enough, feel free to prove my
            // assumptions wrong. I'll be very thankful, but also very sorry for the time you wasted
            // performing that pointless microoptimization. Just like you might be sorry for the
            // time I wasted writing a whole wall of text just to explain why I used one instruction
            // instead of two.
            self.0.out_of_instancer.store(false, Release)
        }
    }
}

impl<T: Debug> Debug for InstanceInner<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instance") // Not deriving to override struct name
            .field("inner", &self.instance)
            .field("split", &self.split)
            .field("out_of_instancer", &self.out_of_instancer)
            .finish()
    }
}
impl<T: Debug> Debug for Instance<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f) // passthrough
    }
}
