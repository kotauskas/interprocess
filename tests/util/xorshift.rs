use std::time::{SystemTime, UNIX_EPOCH};

/// The 32-bit variant of the Xorshift PRNG algorithm.
///
/// Didn't feel like pulling in the `rand` crate, so have this here beauty instead.
#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct Xorshift32(pub u32);
impl Xorshift32 {
    pub fn from_system_time() -> Self {
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|e| e.duration());
        Self(dur.subsec_nanos())
    }
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}
impl Iterator for Xorshift32 {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next())
    }
}
