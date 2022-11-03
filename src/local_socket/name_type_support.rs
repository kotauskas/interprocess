impmod! {local_socket,
    name_type_support_query as name_type_support_query_impl,
    NAME_TYPE_ALWAYS_SUPPORTED as NAME_TYPE_ALWAYS_SUPPORTED_REAL,
}

/// Represents which kinds of identifiers can be used for a local socket's name on the current platform.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NameTypeSupport {
    /// Only filesystem paths can be used.
    ///
    /// This is true for all Unix/POSIX and Unix-like systems other than Linux.
    OnlyPaths,
    /// Only names in an dedicated namespace can be used.
    ///
    /// This is true only for Windows.
    OnlyNamespaced,
    /// Both of the above options are available.
    ///
    /// This is true only for Linux.
    Both,
}
impl NameTypeSupport {
    /// The types of local socket names supported on the current platform regardless of the environment and OS version.
    ///
    /// On most platforms, the value is known at compile time, i.e. the support for paths wasn't introduced in a specific version of the OS or isn't known to be supported at all. **Currently, this includes all supported OSes.** If support is added for an OS which added this functionality in a specific version, this constant will be the most restrictive value for that platform, with [`query`] possibly returning the actual value according to the current version of the OS.
    ///
    /// Simply put, you should probably just use this value for consistency across platforms, unless you really need a specific name type to be supported.
    ///
    /// [`query`]: #method.query " "
    pub const ALWAYS_AVAILABLE: Self = NAME_TYPE_ALWAYS_SUPPORTED_REAL;
    /// Returns the types of local socket names supported on the current platform with the current environment.
    ///
    /// On most platforms, the value is known at compile time, i.e. the support for one of the types wasn't introduced in an update to the OS or isn't known to be supported at all. **Currently, this includes all supported OSes.** For compatibility with OSes which might add the functionality in the future starting with a specific version, this function isn't a `const fn` – see [`ALWAYS_AVAILABLE`] if you need a constant expression.
    ///
    /// [`ALWAYS_AVAILABLE`]: #associatedconstant.ALWAYS_AVAILABLE " "
    pub fn query() -> Self {
        name_type_support_query_impl()
    }

    /// Returns `true` if, according to `self`, filesystem-based local sockets are supported; `false` otherwise.
    pub const fn paths_supported(self) -> bool {
        matches!(self, Self::OnlyPaths | Self::Both)
    }
    /// Returns `true` if, according to `self`, namespaced local socket names are supported; `false` otherwise.
    pub const fn namespace_supported(self) -> bool {
        matches!(self, Self::OnlyNamespaced | Self::Both)
    }
}
