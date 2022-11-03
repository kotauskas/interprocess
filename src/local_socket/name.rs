use {
    super::NameTypeSupport,
    std::{
        borrow::Cow,
        ffi::{OsStr, OsString},
    },
};

/// A name for a local socket.
///
/// Due to vast differences between platforms in terms of how local sockets are named, there needs to be a way to store and process those in a unified way while also retaining platform-specific pecularities. `LocalSocketName` aims to bridge the gap between portability and platform-specific correctness.
///
/// # Creation
/// A separate trait is used to create names from basic strings: [`ToLocalSocketName`](super::ToLocalSocketName). Aside from being conveniently implemented on every single string type in the standard library, it also provides some special processing. Please read its documentation if you haven't already – the rest of this page assumes you did.
///
/// # Validity
/// As mentioned in the [module-level documentation], not all platforms support all types of local socket names. A name pointing to a filesystem location is only supported on Unix-like systems, and names pointing to an abstract namespace reserved specifically for local sockets are only available on Linux and Windows. Due to the diversity of those differences, `LocalSocketName` does not provide any forced validation by itself – the [`is_supported`] and [`is_always_supported`] checks are not enforced to succeed. Instead, they are intended as helpers for the process of user input validation, if any local socket names are ever read from environment variables, configuration files or other methods of user input.
///
/// If an invalid local socket name is used to create a local socket or connect to it, the creation/connection method will fail.
///
/// [`to_local_socket_name`]: trait.ToLocalSocketName.html " "
/// [module-level documentation]: index.html " "
/// [`is_supported`]: #method.is_supported " "
/// [`is_always_supported`]: #method.is_always_supported " "
pub struct LocalSocketName<'a> {
    inner: Cow<'a, OsStr>,
    namespaced: bool,
}
impl<'a> LocalSocketName<'a> {
    /// Returns `true` if the type of the name is supported by the OS, `false` otherwise.
    ///
    /// The check is performed at runtime. For a conservative compile-time check, see [`.is_always_supported`](Self::is_always_supported).
    pub fn is_supported(&self) -> bool {
        self.is_supported_in_nts_type(NameTypeSupport::query())
    }
    /// Returns `true` if the type of the name is supported by the OS, `false` otherwise.
    ///
    /// The check is performed at compile-time. For a check which might return a more permissive result on certain platforms by checking for support at runtime, see [`.is_supported()`](Self::is_supported).
    pub const fn is_always_supported(&self) -> bool {
        self.is_supported_in_nts_type(NameTypeSupport::ALWAYS_AVAILABLE)
    }
    /// Returns `true` if the type of the name is supported by an OS with the specified name type support class, `false` otherwise.
    ///
    /// This is mainly a helper function for [`.is_supported()`](Self::is_supported) and [`.is_always_supported()`](Self::is_always_supported), but there's no good reason not to expose it as a public method, so why not?
    pub const fn is_supported_in_nts_type(&self, nts: NameTypeSupport) -> bool {
        (self.is_namespaced() && nts.namespace_supported())
            || (self.is_path() && nts.paths_supported())
    }
    /// Returns `true` if the value is a namespaced name, `false` otherwise.
    pub const fn is_namespaced(&self) -> bool {
        self.namespaced
    }
    /// Returns `true` if the value is a filesystem path, `false` otherwise.
    pub const fn is_path(&self) -> bool {
        !self.namespaced
    }
    /// Returns the name as an `OsStr`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as an owned `OsString` instead, see [`.into_inner()`](Self::into_inner).
    pub fn inner(&'a self) -> &'a OsStr {
        &self.inner
    }
    /// Returns the name as an `OsString`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr` instead, see [`.inner()`](Self::inner).
    pub fn into_inner(self) -> OsString {
        self.inner.into_owned()
    }
    /// Returns the name as a *borrowed* `Cow<'_, OsStr>`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr`, see [`.inner()`](Self::inner); if you need the value as an owned `OsString`, see [`.into_inner()`](Self::into_inner). If you need to take ownership of the `Cow`, see [`.into_inner_cow()`](Self::into_inner_cow).
    pub const fn inner_cow(&'a self) -> &'a Cow<'a, OsStr> {
        &self.inner
    }
    /// Returns the name as a `Cow<'_, OsStr>`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr`, see [`inner`]; if you need the value as an owned `OsString`, see [`.into_inner()`](Self::into_inner). If you don't need to take ownership of the `Cow`, see [`.inner_cow()`](Self::inner_cow).
    pub fn into_inner_cow(self) -> Cow<'a, OsStr> {
        self.inner
    }
    pub(crate) const fn from_raw_parts(inner: Cow<'a, OsStr>, namespaced: bool) -> Self {
        Self { inner, namespaced }
    }
}
