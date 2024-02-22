use crate::local_socket::{Name, NameTypeSupport};
use std::{
	ffi::{CStr, CString, OsStr, OsString},
	io,
	os::unix::ffi::{OsStrExt, OsStringExt},
};

pub fn name_type_support_query() -> NameTypeSupport {
	NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(uds_linux_namespace)]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(uds_linux_namespace))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyFs;

pub fn is_namespaced(slf: &Name<'_>) -> bool {
	!slf.is_path()
}

#[inline]
pub fn cstr_to_osstr(cstr: &CStr) -> io::Result<&OsStr> {
	Ok(OsStr::from_bytes(cstr.to_bytes()))
}

#[inline]
pub fn cstring_to_osstring(cstring: CString) -> io::Result<OsString> {
	Ok(OsString::from_vec(cstring.into_bytes()))
}

pub fn is_supported(s: &OsStr, path: bool) -> io::Result<bool> {
	let Some(first) = s.as_bytes().first() else {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"local socket name cannot be empty",
		));
	};
	#[cfg(not(any(target_os = "linux", target_os = "android")))]
	{
		if !path {
			return Ok(false);
		}
	}
	let begnul = *first == b'\0';
	if path && begnul {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"\
filesystem paths cannot have interior nuls (only nuls in the first byte are reported by this check)",
		));
	}
	Ok(true)
}
