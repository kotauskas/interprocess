use super::{
    imports::*,
    util::{empty_cstr, empty_cstring, eunreachable},
    MAX_UDSOCKET_PATH_LEN,
};
use std::{
    borrow::{Cow, ToOwned},
    convert::TryFrom,
    ffi::{CStr, CString, NulError, OsStr, OsString},
    io,
    mem::{replace, size_of_val, zeroed},
    ops::Deref,
    path::{Path, PathBuf},
    ptr,
};

/// Represents a name for a Unix domain socket.
///
/// The main purpose for this enumeration is to conditionally support the dedicated socket namespace on systems which implement it – for that, the `Namespaced` variant is used. Depending on your system, you might not be seeing it, which is when you'd need the `File` fallback variant, which works on all POSIX-compliant systems.
///
/// ## `Namespaced`
/// This variant refers to sockets in a dedicated socket namespace, which is fully isolated from the main filesystem and closes sockets automatically when the server which opened the socket shuts down. **This variant is only implemented on Linux, which is why it is not available on other POSIX-conformant systems at compile time, resulting in a compile-time error if usage is attempted.**
///
/// ## `File`
/// All sockets identified this way are located on the main filesystem and exist as persistent files until deletion, preventing servers from using the same socket without deleting it from the filesystem first. This variant is available on all POSIX-compilant systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UdSocketPath<'a> {
    /// An unnamed socket, identified only by its file descriptor. This is an invalid path value for creating sockets – all attempts to use such a value will result in an error.
    Unnamed,
    /// Identifies a socket which is located in the filesystem tree, existing as a file. See the [enum-level documentation] for more.
    ///
    /// [enum-level documentation]: #file " "
    File(Cow<'a, CStr>),
    /// Identifies a socket in the dedicated socket namespace, where it exists until the server closes it rather than persisting as a file. See the [enum-level documentation] for more.
    ///
    /// [enum-level documentation]: #namespaced " "
    #[cfg(uds_linux_namespace)]
    #[cfg_attr( // uds_linux_namespace template
        feature = "doc_cfg",
        doc(cfg(any(target_os = "linux", target_os = "android")))
    )]
    Namespaced(Cow<'a, CStr>),
}
impl<'a> UdSocketPath<'a> {
    /// Returns the path as a [`CStr`]. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    pub fn as_cstr(&'a self) -> &'a CStr {
        match self {
            Self::File(cow) => cow.deref(),
            #[cfg(uds_linux_namespace)]
            Self::Namespaced(cow) => cow.deref(),
            Self::Unnamed => empty_cstr(),
        }
    }
    /// Returns the path as an [`OsStr`]. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    pub fn as_osstr(&'a self) -> &'a OsStr {
        OsStr::from_bytes(self.as_cstr().to_bytes())
    }
    /// Returns the path as a [`CString`]. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    pub fn into_cstring(self) -> CString {
        match self {
            Self::File(cow) => cow.into_owned(),
            #[cfg(uds_linux_namespace)]
            Self::Namespaced(cow) => cow.into_owned(),
            Self::Unnamed => empty_cstring(),
        }
    }
    /// Returns the path as an [`OsString`]. The resulting value does not include any indication of whether it's a namespaced socket name or a filesystem path.
    pub fn into_osstring(self) -> OsString {
        OsString::from_vec(self.into_cstring().into_bytes())
    }

    /// Ensures that the path is stored as an owned `CString` in place, and returns whether that required cloning or not. If `self` was not referring to any socket ([`Unnamed` variant]), the value is set to an empty `CString` (only nul terminator) of type [`File`].
    ///
    /// [`Unnamed` variant]: #variant.Unnamed " "
    /// [`File`]: #file " "
    pub fn make_owned(&mut self) -> bool {
        let required_cloning = !self.is_owned();
        *self = self.to_owned();
        required_cloning
    }
    /// Converts to a `UdSocketPath<'static>` which stores the path as an owned `CString`, cloning if necessary.
    // TODO implement ToOwned instead of Clone in 2.0.0
    pub fn to_owned(&self) -> UdSocketPath<'static> {
        match self {
            Self::File(f) => UdSocketPath::File(Cow::Owned(f.as_ref().to_owned())),
            #[cfg(uds_linux_namespace)]
            Self::Namespaced(n) => UdSocketPath::Namespaced(Cow::Owned(n.as_ref().to_owned())),
            Self::Unnamed => UdSocketPath::Unnamed,
        }
    }
    /// Borrows into another `UdSocketPath<'_>` instance. If borrowed here, reborrows; if owned here, returns a fresh borrow.
    pub fn borrow(&self) -> UdSocketPath<'_> {
        match self {
            UdSocketPath::File(f) => UdSocketPath::File(Cow::Borrowed(f.as_ref())),
            #[cfg(uds_linux_namespace)]
            UdSocketPath::Namespaced(n) => UdSocketPath::Namespaced(Cow::Borrowed(n.as_ref())),
            UdSocketPath::Unnamed => UdSocketPath::Unnamed,
        }
    }

    /// Returns a mutable reference to the underlying `CString`, cloning the borrowed path if it wasn't owned before.
    pub fn get_cstring_mut(&mut self) -> &mut CString {
        self.make_owned();
        self.try_get_cstring_mut().unwrap_or_else(|| unsafe {
            // SAFETY: the call to make_owned ensured that there is a CString
            std::hint::unreachable_unchecked()
        })
    }
    /// Returns a mutable reference to the underlying `CString` if it's available as owned, otherwise returns `None`.
    pub fn try_get_cstring_mut(&mut self) -> Option<&mut CString> {
        let cow = match self {
            Self::File(cow) => cow,
            #[cfg(uds_linux_namespace)]
            Self::Namespaced(cow) => cow,
            Self::Unnamed => return None,
        };
        match cow {
            Cow::Owned(cstring) => Some(cstring),
            Cow::Borrowed(..) => None,
        }
    }

    /// Returns `true` if the path to the socket is stored as an owned `CString`, i.e. if `into_cstring` doesn't require cloning the path; `false` otherwise.
    // Cannot use `matches!` due to #[cfg(...)]
    #[allow(clippy::match_like_matches_macro)]
    pub const fn is_owned(&self) -> bool {
        match self {
            Self::File(Cow::Borrowed(..)) => true,
            #[cfg(uds_linux_namespace)]
            Self::Namespaced(Cow::Borrowed(..)) => true,
            _ => false,
        }
    }

    #[cfg(unix)]
    pub(super) fn write_sockaddr_un_to_self(&mut self, addr: &sockaddr_un, addrlen: usize) {
        let sun_path_length = (addrlen as isize) - (size_of_val(&addr.sun_family) as isize);
        let sun_path_length = match usize::try_from(sun_path_length) {
            Ok(val) => val,
            Err(..) => {
                *self = Self::Unnamed;
                return;
            }
        };
        if let Some(cstring) = self.try_get_cstring_mut() {
            let cstring = replace(cstring, empty_cstring());
            let mut vec = cstring.into_bytes_with_nul();
            let mut _namespaced = false;
            unsafe {
                #[cfg(uds_linux_namespace)]
                let (src_ptr, path_length) = if addr.sun_path[0] == 0 {
                    _namespaced = true;
                    (
                        addr.sun_path.as_ptr().offset(1) as *const u8,
                        sun_path_length - 1,
                    )
                } else {
                    (addr.sun_path.as_ptr() as *const u8, sun_path_length)
                };
                #[cfg(not(uds_linux_namespace))]
                let (src_ptr, path_length) =
                    { (addr.sun_path.as_ptr() as *const u8, sun_path_length) };
                // Fill the space for the name and the nul terminator with nuls
                vec.resize(path_length, 0);
                ptr::copy_nonoverlapping(src_ptr, vec.as_mut_ptr(), path_length);
            };
            // If the system added a nul byte as part of the length, remove the one we added ourselves.
            if vec.last() == Some(&0) && vec[vec.len() - 2] == 0 {
                vec.pop();
            }
            let new_cstring = CString::new(vec).unwrap_or_else(eunreachable);
            #[cfg(uds_linux_namespace)]
            let path_to_write = if _namespaced {
                UdSocketPath::Namespaced(Cow::Owned(new_cstring))
            } else {
                UdSocketPath::File(Cow::Owned(new_cstring))
            };
            #[cfg(not(uds_linux_namespace))]
            let path_to_write = UdSocketPath::File(Cow::Owned(new_cstring));
            *self = path_to_write;
            // Implicitly drops the empty CString we wrote in the beginning
        } else {
            let mut _namespaced = false;
            let mut vec = unsafe {
                let (src_ptr, path_length) = if addr.sun_path[0] == 0 {
                    (
                        addr.sun_path.as_ptr().offset(1) as *const u8,
                        sun_path_length - 1,
                    )
                } else {
                    (addr.sun_path.as_ptr() as *const u8, sun_path_length)
                };
                let mut vec = vec![0; path_length];
                ptr::copy_nonoverlapping(src_ptr, vec.as_mut_ptr(), path_length);
                vec
            };
            // If the system added a nul byte as part of the length, remove it.
            if vec.last() == Some(&0) {
                vec.pop();
            }
            let cstring = CString::new(vec).unwrap_or_else(eunreachable);
            #[cfg(uds_linux_namespace)]
            let path_to_write = if _namespaced {
                UdSocketPath::Namespaced(Cow::Owned(cstring))
            } else {
                UdSocketPath::File(Cow::Owned(cstring))
            };
            #[cfg(not(uds_linux_namespace))]
            let path_to_write = UdSocketPath::File(Cow::Owned(cstring));
            *self = path_to_write;
        }
    }
    /// Returns `addr_len` to pass to `bind`/`connect`.
    #[cfg(unix)]
    pub(super) fn write_self_to_sockaddr_un(&self, addr: &mut sockaddr_un) -> io::Result<()> {
        let is_namespaced;
        let len_of_self = self.as_cstr().to_bytes_with_nul().len();
        match self {
            UdSocketPath::File(..) => {
                is_namespaced = false;
                if len_of_self > MAX_UDSOCKET_PATH_LEN {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "socket path should not be longer than {} bytes",
                            MAX_UDSOCKET_PATH_LEN
                        ),
                    ));
                }
            }
            #[cfg(uds_linux_namespace)]
            UdSocketPath::Namespaced(..) => {
                is_namespaced = true;
                if len_of_self > (MAX_UDSOCKET_PATH_LEN - 1) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "namespaced socket name should not be longer than {} bytes",
                            MAX_UDSOCKET_PATH_LEN - 1
                        ),
                    ));
                }
            }
            UdSocketPath::Unnamed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "must provide a name for the socket",
                ))
            }
        }

        unsafe {
            ptr::copy_nonoverlapping(
                self.as_cstr().as_ptr(),
                if is_namespaced {
                    addr.sun_path.as_mut_ptr().offset(1)
                } else {
                    addr.sun_path.as_mut_ptr()
                },
                len_of_self,
            );
        }
        Ok(())
    }
}
impl UdSocketPath<'static> {
    /// Creates a buffer suitable for usage with [`recv_from`] ([`_ancillary`]/[`_vectored`]/[`_ancillary_vectored`]). The capacity is equal to the [`MAX_UDSOCKET_PATH_LEN`] constant (the nul terminator in the `CString` is included). **The contained value is unspecified – results of reading from the buffer should not be relied upon.**
    ///
    /// # Example
    /// ```
    /// # #[cfg(unix)] {
    /// use interprocess::os::unix::udsocket::{UdSocketPath, MAX_UDSOCKET_PATH_LEN};
    /// use std::borrow::Cow;
    ///
    /// let path_buffer = UdSocketPath::buffer();
    /// match path_buffer {
    ///     UdSocketPath::File(cow) => match cow {
    ///         Cow::Owned(cstring)
    ///     => assert_eq!(cstring.into_bytes_with_nul().capacity(), MAX_UDSOCKET_PATH_LEN),
    ///         Cow::Borrowed(..) => unreachable!(),
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// # }
    /// ```
    ///
    /// [`recv_from`]: struct.UdSocket.html#method.recv_from " "
    /// [`_ancillary`]: struct.UdSocket.html#method.recv_from " "
    /// [`_vectored`]: struct.UdSocket.html#method.recv_from_vectored " "
    /// [`_ancillary_vectored`]: struct.UdSocket.html#method.recv_from_ancillary_vectored " "
    /// [`MAX_UDSOCKET_PATH_LEN`]: constant.MAX_UDSOCKET_PATH_LEN.html " "
    pub fn buffer() -> Self {
        Self::File(Cow::Owned(
            CString::new(vec![0x2F; MAX_UDSOCKET_PATH_LEN - 1])
                .expect("unexpected nul in newly created Vec, possible heap corruption"),
        ))
    }

    /// Constructs a `UdSocketPath::File` value from a `Vec` of bytes, wrapping `CString::new`.
    pub fn file_from_vec(vec: Vec<u8>) -> Result<Self, NulError> {
        Ok(Self::File(Cow::Owned(CString::new(vec)?)))
    }
    /// Constructs a `UdSocketPath::Namespaced` value from a `Vec` of bytes, wrapping `CString::new`.
    #[cfg(uds_linux_namespace)]
    #[cfg_attr( // uds_linux_namespace template
        feature = "doc_cfg",
        doc(cfg(any(target_os = "linux", target_os = "android")))
    )]
    pub fn namespaced_from_vec(vec: Vec<u8>) -> Result<Self, NulError> {
        Ok(Self::Namespaced(Cow::Owned(CString::new(vec)?)))
    }
}
impl From<UdSocketPath<'_>> for CString {
    fn from(path: UdSocketPath<'_>) -> Self {
        path.into_cstring()
    }
}
impl AsRef<CStr> for UdSocketPath<'_> {
    fn as_ref(&self) -> &CStr {
        self.as_cstr()
    }
}
impl From<UdSocketPath<'_>> for OsString {
    fn from(path: UdSocketPath<'_>) -> Self {
        path.into_osstring()
    }
}
impl AsRef<OsStr> for UdSocketPath<'_> {
    fn as_ref(&self) -> &OsStr {
        self.as_osstr()
    }
}
impl TryFrom<UdSocketPath<'_>> for sockaddr_un {
    type Error = io::Error;
    fn try_from(path: UdSocketPath<'_>) -> io::Result<Self> {
        unsafe {
            let mut addr: sockaddr_un = zeroed();
            addr.sun_family = AF_UNIX as _;
            path.write_self_to_sockaddr_un(&mut addr)?;
            Ok(addr)
        }
    }
}

/// Trait for types which can be converted to a [path to a Unix domain socket][`UdSocketPath`].
///
/// The difference between this trait and [`TryInto`]`<`[`UdSocketPath`]`>` is that the latter does not constrain the error type to be [`io::Error`] and thus is not compatible with many types from the standard library which are widely expected to be convertible to Unix domain socket paths. Additionally, this makes the special syntax for namespaced sockets possible (see below).
///
/// ## `@` syntax for namespaced paths
/// On Linux (since it's the only platform which supports [namespaced socket paths]), an extra syntax feature is implemented for string types which don't have file path semantics, i.e. all standard string types except for [`Path`] and [`PathBuf`]. If the first character in a string is `@`, the path is interpreted as a namespaced socket path rather than a normal file path. Read the `UdSocketPath` documentation for more on what that means. There are several ways to opt out of that behavior if you're referring to a socket at a relative path which starts from a `@`:
/// - Use [`AsRef`] to convert the string slice type into a [`Path`] which has file path semantics and therefore does not have the `@` syntax enabled, if your string type is [`str`] or [`OsStr`]
/// - Prefix the path with `./`, which carries the same meaning from the perspective of the OS but bypasses the `@` check
/// - If your string type is [`CStr`] or [`CString`], explicitly construct `UdSocketPath`'s `File` variant with a [`Cow`] wrapping your string value
///
/// # Example
/// The following example uses the `UdStreamListener::bind` method, but `UdStream::connect` and `UdSocket::bind`/`UdSocket::connect` accept the same argument types too.
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(unix)] {
/// use interprocess::os::unix::udsocket::{UdStreamListener, UdSocketPath};
/// use std::{ffi::{CStr, CString}, path::{Path, PathBuf}, borrow::Cow};
///
/// // 1. Use a string literal
/// let listener = UdStreamListener::bind("/tmp/example1.sock")?;
/// // If we're on Linux, we can also use the abstract socket namespace which exists separately from
/// // the filesystem thanks to the special @ sign syntax which works with all string types
/// let listener_namespaced = UdStreamListener::bind("@namespaced_socket_1")?;
///
/// // 2. Use an owned string
/// let listener = UdStreamListener::bind("/tmp/example2.sock".to_string())?;
/// // Same story with the namespaced socket here
/// let listener_namespaced = UdStreamListener::bind("@namespaced_socket_2")?;
///
/// // 3. Use a path slice or an owned path
/// let listener_by_path = UdStreamListener::bind(Path::new("/tmp/exmaple3a.sock"))?;
/// let listener_by_pathbuf = UdStreamListener::bind(PathBuf::from("/tmp/example3b.sock"))?;
/// // The @ syntax doesn't work with Path and PathBuf, since those are explicitly paths at the type
/// // level, rather than strings with contextual meaning. Using AsRef to convert an &str slice or
/// // an &OsStr slice into a &Path slice is the recommended way to disable the @ syntax.
///
/// // 4. Use manual creation
/// let cstring = CString::new("/tmp/example4a.sock".to_string().into_bytes())?;
/// let path_to_socket = UdSocketPath::File(Cow::Owned(cstring));
/// let listener = UdStreamListener::bind(path_to_socket);
///
/// let cstr = CStr::from_bytes_with_nul("/tmp/example4b.sock\0".as_bytes())?;
/// let path_to_socket = UdSocketPath::File(Cow::Borrowed(cstr));
/// let listener = UdStreamListener::bind(path_to_socket);
/// # }
/// # Ok(()) }
/// ```
///
/// [`UdSocketPath`]: enum.UdSocketPath.html " "
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html " "
/// [`TryInto`]: https://doc.rust-lang.org/std/convert/trait.TryInto.html " "
/// [`AsRef`]: https://doc.rust-lang.org/std/convert/trait.AsRef.html " "
/// [namespaced socket paths]: struct.UdSocketPath.html#namespaced " "
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
/// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
/// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
/// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
/// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
/// [`Cow`]: https://doc.rust-lang.org/std/borrow/enum.Cow.html " "
/// [`str`]: https://doc.rust-lang.org/stable/std/primitive.str.html
pub trait ToUdSocketPath<'a> {
    /// Performs the conversion from `self` to a Unix domain socket path.
    #[allow(clippy::wrong_self_convention)]
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>>;
}
impl<'a> ToUdSocketPath<'a> for UdSocketPath<'a> {
    /// Accepts explicit `UdSocketPath`s in relevant constructors.
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        Ok(self)
    }
}
impl<'a> ToUdSocketPath<'a> for &'a UdSocketPath<'a> {
    /// Reborrows an explicit `UdSocketPath` for a smaller lifetime.
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        Ok(self.borrow())
    }
}
impl<'a> ToUdSocketPath<'a> for &'a CStr {
    /// Converts a borrowed [`CStr`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        // 0x40 is the ASCII code for @, and since UTF-8 is ASCII-compatible, it would work too
        #[cfg(uds_linux_namespace)]
        if self.to_bytes().first() == Some(&0x40) {
            let without_at_sign = &self.to_bytes_with_nul()[1..];
            let without_at_sign = unsafe {
                // SAFETY: it's safe to assume that the second byte comes before the nul
                // terminator or is that nul terminator itself if the first one is an @ sign
                CStr::from_bytes_with_nul_unchecked(without_at_sign)
            };
            // Use early return to simplify the conditional inclusion for the @ syntax check.
            return Ok(UdSocketPath::Namespaced(Cow::Borrowed(without_at_sign)));
        }
        Ok(UdSocketPath::File(Cow::Borrowed(self)))
    }
}
impl ToUdSocketPath<'static> for CString {
    /// Converts an owned [`CString`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(uds_linux_namespace)]
        if self.as_bytes().first() == Some(&0x40) {
            let without_at_sign = {
                let mut without_at_sign = self.into_bytes();
                without_at_sign.remove(0);
                unsafe {
                    // SAFETY: see CStr impl for why this is safe in both impls
                    CString::from_vec_unchecked(without_at_sign)
                }
            };
            // As in the CStr impl, we're using an early return to simplify conditional compilation
            return Ok(UdSocketPath::Namespaced(Cow::Owned(without_at_sign)));
        }
        Ok(UdSocketPath::File(Cow::Owned(self)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a OsStr {
    /// Converts a borrowed [`OsStr`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        #[cfg(uds_linux_namespace)]
        if self.as_bytes().first() == Some(&0x40) {
            if self.as_bytes().last() != Some(&0) {
                let mut owned = self.to_owned().into_vec();
                owned.remove(0);
                return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(owned)?)));
            } else {
                let without_at_sign = self.as_bytes().split_at(1).0;
                let cstr = CStr::from_bytes_with_nul(without_at_sign)
                    .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
                return Ok(UdSocketPath::Namespaced(Cow::Borrowed(cstr)));
            }
        }
        if self.as_bytes().last() != Some(&0) {
            Ok(UdSocketPath::File(Cow::Owned(CString::new(
                self.to_owned().into_vec(),
            )?)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for OsString {
    /// Converts a borrowed [`OsString`] to an owned `UdSocketPath`. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`OsString`]: https://doc.rust-lang.org/std/ffi/struct.OsString.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(uds_linux_namespace)]
        if self.as_os_str().as_bytes().first() == Some(&0x40) {
            let mut without_at_sign = self.into_vec();
            without_at_sign.remove(0);
            return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(
                without_at_sign,
            )?)));
        }
        Ok(UdSocketPath::File(Cow::Owned(CString::new(
            self.into_vec(),
        )?)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a Path {
    /// Converts a borrowed [`Path`] to a borrowed [`UdSocketPath::File`] with the same lifetime.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
    /// [`UdSocketPath::File`]: struct.UdSocketPath.html#file " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        if self.as_os_str().as_bytes().last() != Some(&0) {
            let osstring = self.to_owned().into_os_string().into_vec();
            let cstring = CString::new(osstring)?;
            Ok(UdSocketPath::File(Cow::Owned(cstring)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_os_str().as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for PathBuf {
    /// Converts an owned [`PathBuf`] to an owned [`UdSocketPath::File`].
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
    /// [`UdSocketPath::File`]: struct.UdSocketPath.html#file " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        let cstring = CString::new(self.into_os_string().into_vec())?;
        Ok(UdSocketPath::File(Cow::Owned(cstring)))
    }
}
impl<'a> ToUdSocketPath<'a> for &'a str {
    /// Converts a borrowed [`str`] to a borrowed `UdSocketPath` with the same lifetime. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end. This is done to support normal string literals, since adding `\0` at the end of every single socket name string is tedious and unappealing.
    ///
    /// [`str`]: https://doc.rust-lang.org/std/primitive.str.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'a>> {
        // Use chars().next() instead of raw indexing to account for UTF-8 with BOM
        #[cfg(uds_linux_namespace)]
        if self.starts_with('@') {
            if !self.ends_with('\0') {
                let mut owned = self.to_owned();
                owned.remove(0);
                return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(owned)?)));
            } else {
                let without_at_sign = self.split_at(1).0;
                let cstr = CStr::from_bytes_with_nul(without_at_sign.as_bytes())
                    .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
                return Ok(UdSocketPath::Namespaced(Cow::Borrowed(cstr)));
            }
        }
        if !self.ends_with('\0') {
            Ok(UdSocketPath::File(Cow::Owned(CString::new(
                self.to_owned(),
            )?)))
        } else {
            let cstr = CStr::from_bytes_with_nul(self.as_bytes())
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidInput, x))?;
            Ok(UdSocketPath::File(Cow::Borrowed(cstr)))
        }
    }
}
impl ToUdSocketPath<'static> for String {
    /// Converts an owned [`String`] to an owned `UdSocketPath`. On platforms which don't support [namespaced socket paths], the variant is always [`File`]; on Linux, which supports namespaced sockets, an extra check for the `@` character is performed. See the trait-level documentation for more.
    ///
    /// If the provided string is not nul-terminated, a nul terminator is automatically added by copying the string into owned storage and adding a nul byte on its end.
    ///
    /// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html " "
    /// [`File`]: enum.UdSocketPath.html#file " "
    /// [namespaced socket paths]: enum.UdSocketPath.html#namespaced " "
    fn to_socket_path(self) -> io::Result<UdSocketPath<'static>> {
        #[cfg(uds_linux_namespace)]
        if self.starts_with('@') {
            let mut without_at_sign = self;
            without_at_sign.remove(0);
            return Ok(UdSocketPath::Namespaced(Cow::Owned(CString::new(
                without_at_sign.into_bytes(),
            )?)));
        }
        Ok(UdSocketPath::File(Cow::Owned(CString::new(
            self.into_bytes(),
        )?)))
    }
}
