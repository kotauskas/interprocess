//! Adapter module, implements local sockets under Windows.

use {
    crate::local_socket::{LocalSocketName, NameTypeSupport},
    std::{
        borrow::Cow,
        ffi::{OsStr, OsString},
    },
};

#[cfg(feature = "tokio_support")]
pub mod tokio;

mod listener;
pub use listener::*;

mod stream;
pub use stream::*;

fn thunk_broken_pipe_to_eof(r: std::io::Result<usize>) -> std::io::Result<usize> {
    match r {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(0),
        els => els,
    }
}

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
fn has_pipefs_prefix(
    val: impl IntoIterator<Item = u16>,
) -> bool {
    let pipefs_prefix: [u16; 9] = [
        // The string \\.\pipe\ in UTF-16
        0x005c, 0x005c, 0x002e, 0x005c, 0x0070, 0x0069, 0x0070, 0x0065, 0x005c,
    ];
    pipefs_prefix.iter().copied().eq(val)

}*/

// TODO add Path/PathBuf special-case for \\.\pipe\*
