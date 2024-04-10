use crate::local_socket::{Name, NameType, NamespacedNameType, PathNameType};
use std::{borrow::Cow, ffi::OsStr, io, os::unix::ffi::OsStrExt, path::Path};

tag_enum!(
/// [Mapping](NameType) that produces local socket names referring to Unix domain sockets bound to
/// the filesystem.
///
/// For Unix domain sockets residing in the Linux abstract namespace, see `AbstractNsUdSocket`
/// instead.
FilesystemUdSocket);
impl NameType for FilesystemUdSocket {
	fn is_supported() -> bool {
		true
	}
}
impl PathNameType for FilesystemUdSocket {
	#[inline]
	fn map(path: Cow<'_, Path>) -> io::Result<Name<'_>> {
		for b in path.as_os_str().as_bytes() {
			if *b == 0 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidInput,
					"filesystem paths cannot contain interior nuls",
				));
			}
		}
		Ok(Name::path(path))
	}
}

tag_enum!(
/// [Mapping](NameType) that produces local socket names referring to Unix domain sockets bound to
/// the Linux abstract namespace.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[cfg_attr(feature = "cfg_doc", doc(cfg(any(target_os = "linux", target_os = "android"))))]
AbstractNsUdSocket);
impl NameType for AbstractNsUdSocket {
	fn is_supported() -> bool {
		// TODO maybe check Linux version here
		true
	}
}
impl NamespacedNameType for AbstractNsUdSocket {
	#[inline]
	fn map(name: Cow<'_, OsStr>) -> io::Result<Name<'_>> {
		Ok(Name::nonpath(name))
	}
}

pub(crate) fn map_generic_path(path: Cow<'_, Path>) -> io::Result<Name<'_>> {
	FilesystemUdSocket::map(path)
}

pub(crate) fn map_generic_namespaced(name: Cow<'_, OsStr>) -> io::Result<Name<'_>> {
	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		AbstractNsUdSocket::map(name)
	}
	#[cfg(not(any(target_os = "linux", target_os = "android")))]
	{
		todo!()
	}
}

pub(crate) fn is_namespaced(slf: &Name<'_>) -> bool {
	!slf.is_path()
}
