impmod! {local_socket::name,
	name_type_support_query as name_type_support_query_impl,
	NAME_TYPE_ALWAYS_SUPPORTED as NAME_TYPE_ALWAYS_SUPPORTED_REAL,
}

// TODO revamp to bitflags..?
/// The ways a local socket's name can be specified on the current platform.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NameTypeSupport {
	/// Most filesystem locations can be used, but there is no non-file-like dedicated namespace.
	///
	/// This is true for all Unix-like systems other than Linux.
	OnlyFs,
	/// Only names in a dedicated namespace can be used. This dedicated namespace may or may not be
	/// a special directory/drive/section on the filesystem.
	///
	/// This is true only for Windows.
	OnlyNs,
	/// Both of the above options are available.
	///
	/// This is true only for Linux.
	Both,
}
impl NameTypeSupport {
	/// The types of local socket names supported on the current platform regardless of the
	/// environment and OS version.
	///
	/// On most platforms, the value is known at compile time, i.e. the support for paths wasn't
	/// introduced in a specific version of the OS or isn't known to be supported at all.
	/// **Currently, this includes all supported OSes.** If support is added for an OS which added
	/// this functionality in a specific version, this constant will be the most restrictive value
	/// for that platform, with [`query`](Self::query) possibly returning the actual value according
	/// to the current version of the OS.
	///
	/// Simply put, you should probably just use this value for consistency across platforms, unless
	/// you really need a specific name type to be supported.
	pub const ALWAYS_AVAILABLE: Self = NAME_TYPE_ALWAYS_SUPPORTED_REAL;
	/// Returns the types of local socket names supported on the current platform with the current
	/// environment.
	///
	/// On most platforms, the value is known at compile time, i.e. the support for one of the types
	/// wasn't introduced in an update to the OS or isn't known to be supported at all. **Currently,
	/// this includes all supported OSes.** For compatibility with OSes which might add the
	/// functionality in the future starting with a specific version, this function isn't a `const
	/// fn` – see [`ALWAYS_AVAILABLE`](Self::ALWAYS_AVAILABLE) if you need a constant expression.
	pub fn query() -> Self {
		name_type_support_query_impl()
	}

	/// Returns `true` if, according to `self`, filesystem-based local sockets are supported;
	/// `false` otherwise.
	pub const fn fs_supported(self) -> bool {
		matches!(self, Self::OnlyFs | Self::Both)
	}
	/// Returns `true` if, according to `self`, namespaced local socket names are supported; `false`
	/// otherwise.
	pub const fn ns_supported(self) -> bool {
		matches!(self, Self::OnlyNs | Self::Both)
	}
}
