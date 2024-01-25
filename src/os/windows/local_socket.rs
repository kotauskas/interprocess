//! Adapter module, implements local sockets under Windows.

mod listener;
mod stream;

pub use {listener::*, stream::*};

#[cfg(feature = "tokio")]
pub mod tokio {
    mod listener;
    mod stream;
    pub use {listener::*, stream::*};
}

use crate::local_socket::{LocalSocketName, NameTypeSupport};
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNamespaced;

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
pub fn to_local_socket_name_osstr(osstr: &OsStr) -> LocalSocketName<'_> {
    LocalSocketName::from_raw_parts(Cow::Borrowed(osstr), true)
}
pub fn to_local_socket_name_osstring(osstring: OsString) -> LocalSocketName<'static> {
    LocalSocketName::from_raw_parts(Cow::Owned(osstring), true)
}

/*
/// Helper function to check whether a series of UTF-16 bytes starts with `\\.\pipe\`.
fn has_pipefs_prefix(val: impl IntoIterator<Item = u16>) -> bool {
    const BKSLSH: u16 = '\\' as _;
    const PERIOD: u16 = '.' as _;
    const P: u16 = 'p' as _;
    const I: u16 = 'i' as _;
    const E: u16 = 'e' as _;
    static PIPEFS_PREFIX: [u16; 9] = [BKSLSH, BKSLSH, PERIOD, BKSLSH, P, I, P, E, BKSLSH];
    PIPEFS_PREFIX.iter().copied().eq(val)
}*/

// TODO add Path/PathBuf special-case for \\.\pipe\*
// Maybe use namespaced = false to signify that \\.\pipe\ does not need to be prepended.
