use super::*;

/// Types of backends that can implement local sockets.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImplType {
    /// Local sockets implemented on top of Unix-domain sockets (Unix domain sockets).
    ///
    /// ## Implementation behavior
    /// - Closing the connection on one side does not immediately destroy buffers, allowing one side to read data sent
    ///   by the other side after the latter drops its local socket stream object.
    /// - Vectored I/O is fully supported.
    UdSocket,
    /// Local sockets implemented on top of Windows named pipes.
    ///
    /// ## Implementation behavior
    /// - Closing the connection destroys buffers immediately, erasing everything that's been sent by one side and not
    ///   received by the other and inducing a "broken pipe" error (translated to an EOF condition by `interprocess`
    ///   automatically) if a receive call is attempted.
    /// - Vectored I/O is unsupported.
    WindowsNamedPipe,
}
impl ImplType {
    /// An array of all [`ImplType`]s, for convenient iteration.
    pub const ALL_TYPES: &[ImplType] = &[Self::UdSocket, Self::WindowsNamedPipe];
    /// Returns the [`ImplProperties`] for this implementation type at the lowest guaranteed level on the target
    /// platform, regardless of runtime circumstances (such as the OS version), or `None` if it is not guaranteed to be
    /// supported.
    ///
    /// For example, querying this for `UdSocket` on Windows will always return `None`, since versions of Windows before
    /// Windows 10 update 1803 do not support them.
    pub const fn get_always_supported_properties(self) -> Option<ImplProperties> {
        todo!()
    }
    /// Returns `true` if this implementation type is guaranteed to be supported on the target platform regardless of
    /// runtime circumstances (such as the OS version), `false` otherwise.
    #[inline(always)]
    pub const fn is_always_supported(self) -> bool {
        self.get_always_supported_properties().is_some()
    }
    /// Returns the [`ImplProperties`] for this implementation type in the current runtime circumstances, or `None` if
    /// it is not supported.
    ///
    /// For example, querying this for `UdSocket` on Windows will return `Some(...)` on Windows 10 update 1803 and
    /// later, unlike [`get_always_supported_properties()`] which will err on the side of caution and state that not all
    /// Windows systems support Unix-domain sockets.
    pub fn get_properties(self) -> Option<ImplProperties> {
        todo!()
    }
    /// Returns `true` if this implementation type is supported by the OS in the current runtime circumstances, `false`
    /// otherwise.
    #[inline(always)]
    pub fn is_supported(self) -> bool {
        self.get_properties().is_some()
    }
}

/// A description of a local socket implementation's properties.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ImplProperties {
    pub(crate) name_type_support: NameTypeSupport,
    pub(crate) buffer_retention: bool,
}
impl ImplProperties {
    /// Returns the [`NameTypeSupport`] for this implementation.
    #[inline(always)]
    pub const fn name_type_support(self) -> NameTypeSupport {
        self.name_type_support
    }
    /// Returns `true` if the implementation retains data that has been sent by one side but not received by the other,
    /// or `false` if it discards all in-flight data immediately when the sender drops their stream object, causing an
    /// instant EOF for reads on the other side.
    #[inline(always)]
    pub const fn are_buffers_retained(self) -> bool {
        self.buffer_retention
    }
}
