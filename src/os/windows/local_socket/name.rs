use crate::local_socket::{Name, NameTypeSupport};
use std::{
	ffi::{CStr, CString, OsStr, OsString},
	io, str,
};

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNs;
pub fn name_type_support_query() -> NameTypeSupport {
	NAME_TYPE_ALWAYS_SUPPORTED
}
pub fn is_namespaced(_: &Name<'_>) -> bool {
	true
}

// TODO use native codepage
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

pub fn is_supported(s: &OsStr, path: bool) -> io::Result<bool> {
	Ok(!path || is_pipefs(s))
}

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)] // minlen check
fn is_pipefs(slf: &OsStr) -> bool {
	const PFX1: &[u8] = br"\\";
	const PFX2: &[u8] = br"\pipe\";
	const LEN_PFX1: usize = PFX1.len();
	const LEN_PFX2: usize = PFX2.len();
	const MINLEN: usize = LEN_PFX1 + LEN_PFX2 + 1;

	let b = slf.as_encoded_bytes();
	if (b.len() < MINLEN) || (&b[..LEN_PFX1] != PFX1) {
		return false;
	}
	let Some(slashidx) = findslash(&b[LEN_PFX1..]) else {
		return false;
	};
	let hostbase = LEN_PFX1 + slashidx;
	&b[hostbase..(hostbase + LEN_PFX2)] == PFX2
}

#[inline]
fn findslash(slice: &[u8]) -> Option<usize> {
	for (i, e) in slice.iter().copied().enumerate() {
		if e == b'\\' {
			return Some(i);
		}
	}
	None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use super::*;
	static NP: &str = r"Именованная труба\yeah";
	#[track_caller]
	fn assert_supported(s: impl AsRef<OsStr>, path: bool) {
		assert!(is_supported(s.as_ref(), path).unwrap());
	}
	fn assert_not_supported(s: impl AsRef<OsStr>, path: bool) {
		assert!(matches!(
			is_supported(s.as_ref(), path),
			Ok(false) | Err(..)
		));
	}

	#[test]
	fn local() {
		assert_supported(format!(r"\\.\pipe\{NP}"), true);
	}
	#[test]
	fn remote() {
		assert_supported(format!(r"\\CHARA\pipe\{NP}"), true);
	}

	#[test]
	fn prepend_local() {
		assert_supported(NP, false);
	}
	#[test]
	fn prepend_remote() {
		assert_supported(NP, false);
	}

	#[test]
	fn bad() {
		assert_not_supported("iwiwiwiwiwiwiwiwiwiwiwiwi", true);
	}
	#[test]
	fn can_not_do_unix_things() {
		assert_not_supported(r"C:\Users\GetSilly\neovide.sock", true);
	}
}
