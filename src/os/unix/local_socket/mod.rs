//! Adapter module, implements local sockets under Unix.

#[cfg(feature = "tokio_support")]
pub mod tokio;

mod listener;
pub use listener::*;

mod stream;
pub use stream::*;

use {
    crate::{
        local_socket::{LocalSocketName, NameTypeSupport},
        os::unix::udsocket::UdSocketPath,
    },
    std::{
        borrow::Cow,
        ffi::{CStr, CString, OsStr, OsString},
        io,
        os::unix::ffi::{OsStrExt, OsStringExt},
    },
};

fn local_socket_name_to_ud_socket_path(name: LocalSocketName<'_>) -> io::Result<UdSocketPath<'_>> {
    fn cow_osstr_to_cstr(osstr: Cow<'_, OsStr>) -> io::Result<Cow<'_, CStr>> {
        match osstr {
            Cow::Borrowed(val) => {
                if val.as_bytes().last() == Some(&0) {
                    Ok(Cow::Borrowed(
                        CStr::from_bytes_with_nul(val.as_bytes())
                            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
                    ))
                } else {
                    let owned = val.to_os_string();
                    Ok(Cow::Owned(CString::new(owned.into_vec())?))
                }
            }
            Cow::Owned(val) => Ok(Cow::Owned(CString::new(val.into_vec())?)),
        }
    }
    #[cfg(uds_linux_namespace)]
    if name.is_namespaced() {
        return Ok(UdSocketPath::Namespaced(cow_osstr_to_cstr(
            name.into_inner_cow(),
        )?));
    }
    Ok(UdSocketPath::File(cow_osstr_to_cstr(
        name.into_inner_cow(),
    )?))
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
