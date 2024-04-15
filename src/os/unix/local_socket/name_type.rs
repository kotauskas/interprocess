use crate::local_socket::{Name, NameInner, NameType, NamespacedNameType, PathNameType};
use std::{borrow::Cow, ffi::OsStr, io, os::unix::prelude::*, path::Path};

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
		Ok(Name(NameInner::UdSocketPath(path)))
	}
}

tag_enum!(
/// [Mapping](NameType) that produces local socket names referring to Unix domain sockets bound to
/// special locations in the filesystems that are interpreted as dedicated namespaces.
///
/// This is the substitute for `AbstractNsUdSocket` on non-Linux Unices, and is the only available
/// [namespaced name type](NamespacedNameType) on those systems.
SpecialDirUdSocket);
impl NameType for SpecialDirUdSocket {
	fn is_supported() -> bool {
		true
	}
}
impl NamespacedNameType for SpecialDirUdSocket {
	#[inline]
	fn map(name: Cow<'_, OsStr>) -> io::Result<Name<'_>> {
		for b in name.as_bytes() {
			if *b == 0 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidInput,
					"special directory-bound names cannot contain interior nuls",
				));
			}
		}
		Ok(Name(NameInner::UdSocketPseudoNs(name)))
	}
}

#[cfg(any(target_os = "linux", target_os = "android"))]
tag_enum!(
/// [Mapping](NameType) that produces local socket names referring to Unix domain sockets bound to
/// the Linux abstract namespace.
#[cfg_attr(feature = "cfg_doc", doc(cfg(any(target_os = "linux", target_os = "android"))))]
AbstractNsUdSocket);
#[cfg(any(target_os = "linux", target_os = "android"))]
impl NameType for AbstractNsUdSocket {
	fn is_supported() -> bool {
		// TODO maybe check Linux version here
		true
	}
}
#[cfg(any(target_os = "linux", target_os = "android"))]
impl NamespacedNameType for AbstractNsUdSocket {
	#[inline]
	fn map(name: Cow<'_, OsStr>) -> io::Result<Name<'_>> {
		let name = match name {
			Cow::Borrowed(b) => Cow::Borrowed(b.as_bytes()),
			Cow::Owned(o) => Cow::Owned(o.into_vec()),
		};
		Ok(Name(NameInner::UdSocketNs(name)))
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
		SpecialDirUdSocket::map(name)
	}
}
