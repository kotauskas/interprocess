use super::Name;
use std::{
	borrow::Cow,
	ffi::{CStr, CString, OsStr, OsString},
	io,
	path::{Path, PathBuf},
	str,
};

impmod! {local_socket::to_name,
	cstr_to_osstr,
	cstring_to_osstring,
	is_supported,
}

macro_rules! trivial_string_impl {
	($trait:ident $mtd:ident for $($tgt:ty => $via:ident :: $ctor:ident),+ $(,)?) => {$(
		impl<'s> $trait<'s> for $tgt {
			#[inline]
			fn $mtd(self) -> io::Result<Name<'s>> {
				$via::$ctor(self).$mtd()
			}
		}
	)+};
}

/// Conversion to a filesystem-type local socket name.
pub trait ToFsName<'s> {
	/// Performs the conversion to a filesystem-type name.
	///
	/// Fails if the resulting name isn't supported by the platform.
	fn to_fs_name(self) -> io::Result<Name<'s>>;
}

/// Conversion to a namespaced local socket name.
pub trait ToNsName<'s> {
	/// Performs the conversion to a namespaced name.
	///
	/// Fails if the resulting name isn't supported by the platform.
	fn to_ns_name(self) -> io::Result<Name<'s>>;
}

#[allow(dead_code)]
fn err(s: &'static str) -> io::Error {
	io::Error::new(io::ErrorKind::Unsupported, s)
}
fn err_fs() -> io::Error {
	#[cfg(windows)]
	{
		err("filesystem local sockets are not available on this platform")
	}
	#[cfg(not(windows))]
	{
		unreachable!()
	}
}
fn err_ns() -> io::Error {
	#[cfg(all(unix, not(target_os = "linux")))]
	{
		err("namespaced local sockets are not available on this platform")
	}
	#[cfg(any(not(unix), target_os = "linux"))]
	{
		unreachable!()
	}
}

fn from_osstr(osstr: &OsStr, path: bool) -> io::Result<Name<'_>> {
	is_supported(osstr, path)?
		.then(|| Name::new(Cow::Borrowed(osstr), path))
		.ok_or_else(if path { err_fs } else { err_ns })
}
fn from_osstring(osstring: OsString, path: bool) -> io::Result<Name<'static>> {
	is_supported(&osstring, path)?
		.then(|| Name::new(Cow::Owned(osstring), path))
		.ok_or_else(if path { err_fs } else { err_ns })
}

impl<'s> ToFsName<'s> for &'s Path {
	#[inline]
	fn to_fs_name(self) -> io::Result<Name<'s>> {
		from_osstr(self.as_os_str(), true)
	}
}
impl<'s> ToFsName<'s> for PathBuf {
	#[inline]
	fn to_fs_name(self) -> io::Result<Name<'s>> {
		from_osstring(self.into_os_string(), true)
	}
}
trivial_string_impl! { ToFsName to_fs_name for
	&'s str		=> Path		::new	,
	String		=> PathBuf	::from	,
	&'s OsStr	=> Path		::new	,
	OsString	=> PathBuf	::from	,
}

/// Will fail on Windows if the string isn't valid UTF-8.
impl<'s> ToFsName<'s> for &'s CStr {
	fn to_fs_name(self) -> io::Result<Name<'s>> {
		cstr_to_osstr(self).and_then(<&OsStr>::to_fs_name)
	}
}
/// Will fail on Windows if the string isn't valid UTF-8.
impl<'s> ToFsName<'s> for CString {
	fn to_fs_name(self) -> io::Result<Name<'s>> {
		cstring_to_osstring(self).and_then(OsString::to_fs_name)
	}
}

impl<'s> ToNsName<'s> for &'s OsStr {
	#[inline]
	fn to_ns_name(self) -> io::Result<Name<'s>> {
		from_osstr(self, false)
	}
}
impl<'s> ToNsName<'s> for OsString {
	#[inline]
	fn to_ns_name(self) -> io::Result<Name<'s>> {
		from_osstring(self, false)
	}
}
trivial_string_impl! { ToNsName to_ns_name for
	&'s str	=> OsStr	::new	,
	String	=> OsString	::from	,
}

/// Will fail on Windows if the string isn't valid UTF-8.
impl<'s> ToNsName<'s> for &'s CStr {
	fn to_ns_name(self) -> io::Result<Name<'s>> {
		cstr_to_osstr(self).and_then(<&OsStr>::to_ns_name)
	}
}
/// Will fail on Windows if the string isn't valid UTF-8.
impl<'s> ToNsName<'s> for CString {
	fn to_ns_name(self) -> io::Result<Name<'s>> {
		cstring_to_osstring(self).and_then(OsString::to_ns_name)
	}
}
