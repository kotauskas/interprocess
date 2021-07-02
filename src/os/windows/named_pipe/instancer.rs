use std::sync::{
    atomic::{
        AtomicBool,
        Ordering::{AcqRel, Relaxed},
    },
    Arc, RwLock,
};

pub type Instance<T> = Arc<(T, AtomicBool)>;

pub struct Instancer<T>(pub RwLock<Vec<Instance<T>>>);
impl<T> Instancer<T> {
    pub fn allocate(&self) -> Option<Instance<T>> {
        let instances = self.0.read().expect("unexpected lock poison");
        for inst in instances.iter() {
            // Try to get ownership of the instance by doing a combined compare+exchange,
            // just like a mutex does.
            let cmpxchg_result = inst.1.compare_exchange(false, true, AcqRel, Relaxed);
            if cmpxchg_result.is_ok() {
                // If the compare+exchange returned Ok, then we successfully took ownership of the
                // instance and we can return it right away.
                return Some(Arc::clone(inst));
            }
            // If not, the instance we tried to claim is already at work and we need to seek a new
            // one, which is what the next iteration will do.
        }
        None
    }
    pub fn add_instance(&self, instance: T) -> Instance<T> {
        let new_instance = Arc::new((instance, AtomicBool::new(false)));
        let new_instance_c = Arc::clone(&new_instance); // Clone before locking to reduce downtime
        let mut instances = self.0.write().expect("unexpected lock poison");
        instances.push(new_instance_c);
        new_instance
    }
}
