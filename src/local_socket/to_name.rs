use {
    super::LocalSocketName,
    std::{
        borrow::Cow,
        ffi::{CStr, CString, OsStr, OsString},
        io,
        path::{Path, PathBuf},
        str,
    },
};

impmod! {local_socket,
    to_local_socket_name_osstr,
    to_local_socket_name_osstring,
}

/// Types which can be converted to a local socket name.
///
/// The difference between this trait and [`TryInto`]`<`[`LocalSocketName`]`>` is that the latter does not constrain the error type to be [`io::Error`] and thus is not compatible with many types from the standard library which are widely expected to be convertible to Unix domain socket paths. Additionally, this makes the special syntax for namespaced sockets possible (see below).
///
/// ## `@` syntax for namespaced paths
/// As mentioned in the [`LocalSocketName` documentation][`LocalSocketName`], there are two types of which local socket names can be: filesystem paths and namespaced names. Those are isolated from each other – there's no portable way to represent one using another, though certain OSes might provide ways to do so – Windows does, for example. To be able to represent both in a platform-independent fashion, a special syntax was implemented in implementations of this trait on types from the standard library: "@ syntax".
///
/// The feature, in its core, is extremely simple: if the first character in a string is the @ character, the value of the string is interpreted and stored as a namespaced name (otherwise, it's treated as a filesystem path); the @ character is then removed from the string (by taking a subslice which dosen't include it if a string slice is being used; for owned strings, it's simply removed from the string by shifting the entire string towards the beginning). **[`Path`] and [`PathBuf`] are not affected at all – those have explicit path semantics and therefore cannot logically represent namespaced names.**
///
/// This feature is extremely useful both when using hardcoded literals and accepting user input for the path, but sometimes you might want to prevent this behavior. In such a case, you have the following possible approaches:
/// - If the string is a [`OsStr`]/[`OsString`], it can be cheaply converted to a [`Path`]/[`PathBuf`], which do not support the @ syntax
/// - If the string is a [`str`]/[`String`], it can be cheaply converted to [`OsStr`]/[`OsString`]; then the above method can be applied
/// - If the string is a [`CStr`]/[`CString`], it can be converted to [`str`]/[`String`] using the following code:
/// ```
/// # use std::{
/// #     str::Utf8Error,
/// #     ffi::{CStr, CString},
/// # };
/// fn cstr_to_str(val: &CStr) -> Result<&str, Utf8Error> {
///     std::str::from_utf8(val.to_bytes_with_nul())
/// }
/// fn cstring_to_string(val: CString) -> String {
///     String::from_utf8_lossy(&val.into_bytes_with_nul()).into()
/// }
/// ```
/// Then, the method for [`str`]/[`String`] can be applied.
///
/// None of the above conversions perform memory allocations – the only expensive one is [`CStr`]/[`CString`] which performs a check for valid UTF-8.
///
/// [`LocalSocketName`]: struct.LocalSocketName.html " "
/// [`TryInto`]: https://doc.rust-lang.org/std/convert/trait.TryInto.html " "
/// [`str`]: https://doc.rust-lang.org/std/primitive.str.html " "
/// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html " "
/// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
/// [`OsString`]: https://doc.rust-lang.org/std/ffi/struct.OsString.html " "
/// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
/// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
/// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
pub trait ToLocalSocketName<'a> {
    /// Performs the conversion to a local socket name.
    #[allow(clippy::wrong_self_convention)] // shut the fuck up
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>>;
}

/// Converts a borrowed [`Path`] to a borrowed file-type [`LocalSocketName`] with the same lifetime.
impl<'a> ToLocalSocketName<'a> for &'a Path {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        Ok(LocalSocketName::from_raw_parts(
            Cow::Borrowed(self.as_os_str()),
            false,
        ))
    }
}
/// Converts an owned [`PathBuf`] to an owned file-type [`LocalSocketName`].
impl ToLocalSocketName<'static> for PathBuf {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        Ok(LocalSocketName::from_raw_parts(
            Cow::Owned(self.into_os_string()),
            false,
        ))
    }
}
/// Converts a borrowed [`OsStr`] to a borrowed [`LocalSocketName`] with the same lifetime. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
impl<'a> ToLocalSocketName<'a> for &'a OsStr {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        Ok(to_local_socket_name_osstr(self))
    }
}
/// Converts an owned [`OsString`] to an owned [`LocalSocketName`]. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
impl ToLocalSocketName<'static> for OsString {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        Ok(to_local_socket_name_osstring(self))
    }
}
/// Converts a borrowed [`str`](prim@str) to a borrowed [`LocalSocketName`] with the same lifetime. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
impl<'a> ToLocalSocketName<'a> for &'a str {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        OsStr::new(self).to_local_socket_name()
    }
}
/// Converts an owned [`String`] to an owned [`LocalSocketName`]. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
impl ToLocalSocketName<'static> for String {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        OsString::from(self).to_local_socket_name()
    }
}
/// Converts a borrowed [`CStr`] to a borrowed [`LocalSocketName`] with the same lifetime. **UTF-8 is assumed and the nul terminator is preserved during conversion**. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
// FIXME chop off the nul
impl<'a> ToLocalSocketName<'a> for &'a CStr {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        str::from_utf8(self.to_bytes_with_nul())
            .map(|x| to_local_socket_name_osstr(OsStr::new(x)))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
/// Converts an owned [`CString`] to an owned [`LocalSocketName`]. **UTF-8 is assumed and the nul terminator is preserved during conversion**. On platforms which don't support namespaced socket names, the result is always a file-type name; on platforms that do, prefixing the name with the `@` character will trim it away and yield a namespaced name instead. See the trait-level documentation for more.
impl ToLocalSocketName<'static> for CString {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        String::from_utf8(self.into_bytes_with_nul())
            .map(|x| to_local_socket_name_osstring(OsString::from(x)))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
