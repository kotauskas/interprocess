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

use crate::local_socket::NameTypeSupport;
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    io, str,
};

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNamespaced;

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}

pub fn cstr_to_osstr(cstr: &CStr) -> io::Result<&OsStr> {
    str::from_utf8(cstr.to_bytes())
        .map(OsStr::new)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn cstring_to_osstring(cstring: CString) -> io::Result<OsString> {
    String::from_utf8(cstring.into_bytes())
        .map(OsString::from)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
