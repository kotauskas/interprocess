//! Adapter module, implements local sockets under Unix.

#[cfg(feature = "tokio")]
pub mod tokio;

mod listener;

pub use listener::*;

mod stream;
pub use stream::*;

use crate::local_socket::{LocalSocketName, NameTypeSupport};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::linux::net::SocketAddrExt;
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
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

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(uds_linux_namespace)]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(uds_linux_namespace))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyPaths;

pub fn to_local_socket_name_osstr(mut val: &OsStr) -> LocalSocketName<'_> {
    let mut namespaced = false;
    if let Some(b'@') = val.as_bytes().first().copied() {
        if val.len() >= 2 {
            val = OsStr::from_bytes(&val.as_bytes()[1..]);
        } else {
            val = OsStr::from_bytes(&[]);
        }
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Borrowed(val), namespaced)
}
pub fn to_local_socket_name_osstring(mut val: OsString) -> LocalSocketName<'static> {
    let mut namespaced = false;
    if let Some(b'@') = val.as_bytes().first().copied() {
        let new_val = {
            let mut vec = val.into_vec();
            vec.remove(0);
            OsString::from_vec(vec)
        };
        val = new_val;
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Owned(val), namespaced)
}
