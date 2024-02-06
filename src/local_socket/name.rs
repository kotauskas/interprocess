use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fmt::Debug,
};

impmod! {
    local_socket,
    is_namespaced,
}

// TODO maybe emulate NS on FS-only via tmpfs?
// TODO better PartialEq

// TODO adjust docs
/// A name for a local socket.
///
/// Due to vast differences between platforms in terms of how local sockets are named, there needs
/// to be a way to store and process those in a unified way while also retaining platform-specific
/// pecularities. `LocalSocketName` aims to bridge the gap between portability and platform-specific
/// correctness.
///
/// # Creation
/// Two traits are used to create names from basic strings: [`ToFsName`](super::ToFsName) and
/// [`ToNsName`](super::ToNsName).
///
/// # Validity
/// As mentioned in the [module-level documentation](super), not all platforms support all types of
/// local socket names. Names pointing to filesystem locations are only supported on Unix-like
/// systems, and names pointing to an abstract namespace reserved specifically for local sockets are
/// only available on Linux and Windows.
// TODO document automatic checks
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSocketName<'s> {
    raw: Cow<'s, OsStr>,
    path: bool,
}
impl<'s> LocalSocketName<'s> {
    // TODO get rid of those

    /// Returns `true` if the name points to the dedicated local socket namespace, `false`
    /// otherwise.
    #[inline]
    pub fn is_namespaced(&self) -> bool {
        is_namespaced(self)
    }

    /// Returns `true` if the name is stored as a filesystem path, `false` otherwise.
    ///
    /// Note that it is possible for [`.is_namespaced()`](Self::is_namespaced) and `.is_path()` to
    /// return `true` simultaneously:
    /// ```
    /// # #[cfg(windows)] {
    /// # use interprocess::local_socket::{LocalSocketName, ToFsName, ToNsName};
    /// let name = r"\\.\pipe\example".to_fs_name().unwrap();
    /// assert!(name.is_namespaced()); // \\.\pipe\ is a namespace
    /// assert!(name.is_path());       // \\.\pipe\example is a path
    /// }
    /// ```
    #[inline]
    pub const fn is_path(&self) -> bool {
        self.path
    }

    /// Returns the `OsStr` part of the name's internal representation.
    ///
    /// The returned value might reflect the type of the name (whether it was a filesystem path or a
    /// namespaced name) in some situations on some platforms, namely on Linux, or it might not.
    /// Additionally, two equal `LocalSocketName`s may or may not have their outputs of `.raw()`
    /// compare equal, and vice versa.
    ///
    /// If you need the value as an owned `OsString` instead, use [`.into_raw()`](Self::into_raw).
    #[inline]
    pub fn raw(&'s self) -> &'s OsStr {
        &self.raw
    }

    /// Returns the `OsStr` part of the name's internal representation as an `OsString`, cloning if
    /// necessary. See [`.raw()`](Self::raw()).
    #[inline]
    pub fn into_raw(self) -> OsString {
        self.raw.into_owned()
    }

    /// Returns the `OsStr` part of the name's internal representation as a *borrowed*
    /// `Cow<'_, OsStr>`. See [`.raw()`](Self::raw()).
    #[inline]
    pub const fn raw_cow(&'s self) -> &'s Cow<'s, OsStr> {
        &self.raw
    }

    /// Consumes `self` and returns the `OsStr` part of the name's internal representation as a
    /// `Cow<'_,OsStr>` without cloning. See [`.raw()`](Self::raw()).
    #[inline]
    pub fn into_raw_cow(self) -> Cow<'s, OsStr> {
        self.raw
    }

    /// Produces a `LocalSocketName` that borrows from `self`.
    #[inline]
    pub fn borrow(&self) -> LocalSocketName<'_> {
        LocalSocketName {
            raw: Cow::Borrowed(&self.raw),
            path: self.path,
        }
    }

    /// Extends the lifetime to `'static`, cloning if necessary.
    pub fn into_owned(self) -> LocalSocketName<'static> {
        LocalSocketName {
            raw: Cow::Owned(self.raw.into_owned()),
            path: self.path,
        }
    }

    pub(crate) const fn new(raw: Cow<'s, OsStr>, path: bool) -> Self {
        Self { raw, path }
    }
}
