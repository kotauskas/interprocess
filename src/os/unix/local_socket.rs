//! Adapter module, implements local sockets under Unix.

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
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::linux::net::SocketAddrExt;
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    io,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        net::SocketAddr,
    },
    path::Path,
};

fn name_to_addr(name: LocalSocketName<'_>) -> io::Result<SocketAddr> {
    let _is_ns = name.is_namespaced();
    let name = name.into_inner_cow();
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if _is_ns {
        return SocketAddr::from_abstract_name(name.as_bytes());
    }
    SocketAddr::from_pathname(Path::new(&name))
}

#[derive(Clone, Debug, Default)]
struct ReclaimGuard(Option<LocalSocketName<'static>>);
impl ReclaimGuard {
    fn new(name: LocalSocketName<'static>) -> Self {
        Self(if name.is_path() { Some(name) } else { None })
    }
    fn take(&mut self) -> Self {
        Self(self.0.take())
    }
    fn forget(&mut self) {
        self.0 = None;
    }
}
impl Drop for ReclaimGuard {
    fn drop(&mut self) {
        if let Self(Some(name)) = self {
            if name.is_namespaced() {
                let _ = std::fs::remove_file(name.inner());
            }
        }
    }
}

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(uds_linux_namespace)]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(uds_linux_namespace))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyPaths;

#[inline]
pub fn cstr_to_osstr(cstr: &CStr) -> io::Result<&OsStr> {
    Ok(OsStr::from_bytes(cstr.to_bytes()))
}

#[inline]
pub fn cstring_to_osstring(cstring: CString) -> io::Result<OsString> {
    Ok(OsString::from_vec(cstring.into_bytes()))
}
