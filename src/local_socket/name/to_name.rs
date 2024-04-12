use super::{
	r#type::{NamespacedNameType, PathNameType},
	Name,
};
use std::{
	borrow::Cow,
	ffi::{OsStr, OsString},
	io,
	path::{Path, PathBuf},
	str,
};

// TODO reenable CStr stuff

macro_rules! trivial_string_impl {
	($cvttrait:ident :: $mtd:ident<$nttrait:ident> for $($tgt:ty => $via:ident :: $ctor:ident),+ $(,)?) => {$(
		impl<'s> $cvttrait<'s> for $tgt {
			#[inline]
			fn $mtd<T: $nttrait>(self) -> io::Result<Name<'s>> {
				$via::$ctor(self).$mtd::<T>()
			}
		}
	)+};
}

/// Conversion to a filesystem path-type local socket name.
pub trait ToFsName<'s> {
	/// Performs the conversion to a filesystem path-type name.
	///
	/// Fails if the resulting name isn't supported by the platform.
	fn to_fs_name<NT: PathNameType>(self) -> io::Result<Name<'s>>;
}

/// Conversion to a namespaced local socket name.
pub trait ToNsName<'s> {
	/// Performs the conversion to a namespaced name.
	///
	/// Fails if the resulting name isn't supported by the platform.
	fn to_ns_name<NT: NamespacedNameType>(self) -> io::Result<Name<'s>>;
}

#[allow(dead_code)]
fn err(s: &'static str) -> io::Error {
	io::Error::new(io::ErrorKind::Unsupported, s)
}

impl<'s> ToFsName<'s> for &'s Path {
	#[inline]
	fn to_fs_name<FT: PathNameType>(self) -> io::Result<Name<'s>> {
		FT::map(Cow::Borrowed(self))
	}
}
impl<'s> ToFsName<'s> for PathBuf {
	#[inline]
	fn to_fs_name<FT: PathNameType>(self) -> io::Result<Name<'s>> {
		FT::map(Cow::Owned(self))
	}
}
trivial_string_impl! { ToFsName::to_fs_name<PathNameType> for
	&'s str		=> Path		::new	,
	String		=> PathBuf	::from	,
	&'s OsStr	=> Path		::new	,
	OsString	=> PathBuf	::from	,
}

impl<'s> ToNsName<'s> for &'s OsStr {
	#[inline]
	fn to_ns_name<NT: NamespacedNameType>(self) -> io::Result<Name<'s>> {
		NT::map(Cow::Borrowed(self))
	}
}
impl<'s> ToNsName<'s> for OsString {
	#[inline]
	fn to_ns_name<NT: NamespacedNameType>(self) -> io::Result<Name<'s>> {
		NT::map(Cow::Owned(self))
	}
}
trivial_string_impl! { ToNsName::to_ns_name<NamespacedNameType> for
	&'s str	=> OsStr	::new	,
	String	=> OsString	::from	,
}
