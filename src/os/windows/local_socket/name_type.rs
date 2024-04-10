use crate::local_socket::{Name, NameType, PathNameType};
use std::{borrow::Cow, ffi::OsStr, io, path::Path};

tag_enum!(
/// [Mapping](NameType) that produces
/// [named pipe local socket](crate::os::windows::named_pipe::local_socket) names.
///
/// Named pipe paths of the form `\\HOSTNAME\pipe\PIPENAME` are passed through verbatim. Other paths
/// yield an error, as they do not point to NPFS.
///
/// Namespaced strings have `\\.\pipe\` prepended to them – using
/// [`ToNsName`](crate::local_socket::ToNsName) conversions implies the hostname `.`, which is the
/// local system.
NamedPipe);
impl NameType for NamedPipe {
	fn is_supported() -> bool {
		true
	}
}
impl PathNameType for NamedPipe {
	fn map(path: Cow<'_, Path>) -> io::Result<Name<'_>> {
		if !is_pipefs(path.as_os_str()) {
			return Err(io::Error::new(
				io::ErrorKind::Unsupported,
				"not a named pipe path",
			));
		}
		Ok(Name::path(path))
	}
}

pub(crate) fn is_namespaced(_: &Name<'_>) -> bool {
	true
}

pub(crate) fn map_generic_path(path: Cow<'_, Path>) -> io::Result<Name<'_>> {
	// TODO do something meaningful for non-NPFS paths instead of rejecting them
	NamedPipe::map(path)
}

pub(crate) fn map_generic_namespaced(name: Cow<'_, OsStr>) -> io::Result<Name<'_>> {
	// The prepending currently happens at a later point.
	Ok(Name::nonpath(name))
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
	fn assert_pipefs(s: impl AsRef<OsStr>) {
		assert!(is_pipefs(s.as_ref()));
	}
	#[track_caller]
	fn assert_not_pipefs(s: impl AsRef<OsStr>) {
		assert!(!is_pipefs(s.as_ref()));
	}

	#[test]
	fn local() {
		assert_pipefs(format!(r"\\.\pipe\{NP}"));
	}
	#[test]
	fn remote() {
		assert_pipefs(format!(r"\\CHARA\pipe\{NP}"));
	}

	#[test]
	fn bad() {
		assert_not_pipefs("iwiwiwiwiwiwiwiwiwiwiwiwi");
	}
	#[test]
	fn can_not_do_unix_things() {
		assert_not_pipefs(r"C:\Users\GetSilly\neovide.sock");
	}
}
