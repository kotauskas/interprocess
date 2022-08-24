/// A utility trait that, if used as a supertrait, prevents other crates from implementing the trait.
// If the trait itself was pub(crate), it wouldn't work as a supertrait on public traits. We use a
// private module instead to make it impossible to name the trait from outside the crate.
pub trait Sealed {}
