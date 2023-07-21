// TODO async version

#[cfg(feature = "async")]
pub(super) mod poll {
    #[inline(always)]
    fn assert_future<F: core::future::Future>(fut: F) -> F {
        fut
    }

    mod read;
    mod write;
    pub use {read::*, write::*};

    pub mod futures;
}
pub(super) mod sync {
    mod read;
    mod write;
    pub use {read::*, write::*};
}

#[cfg(feature = "async")]
pub use poll::*;
pub use sync::*;

use std::ops::{Add, AddAssign};

/// The successful result of an ancillary-enabled read.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ReadAncillarySuccess {
    /// How many bytes were read to the main buffer.
    pub main: usize,
    /// How many bytes were read to the ancillary buffer.
    pub ancillary: usize,
}
impl Add for ReadAncillarySuccess {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            main: self.main + rhs.main,
            ancillary: self.ancillary + rhs.ancillary,
        }
    }
}
impl AddAssign for ReadAncillarySuccess {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

fn devector<'a>(bufs: &'a [std::io::IoSlice<'_>]) -> &'a [u8] {
    bufs.iter().find(|b| !b.is_empty()).map_or(&[][..], |b| &**b)
}
fn devector_mut<'a>(bufs: &'a mut [std::io::IoSliceMut<'_>]) -> &'a mut [u8] {
    bufs.iter_mut()
        .find(|b| !b.is_empty())
        .map_or(&mut [][..], |b| &mut **b)
}
