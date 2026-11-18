//! Unix-specific local socket features.

pub(crate) mod dispatch_sync;
#[cfg(feature = "tokio")]
pub(crate) mod dispatch_tokio;
pub(crate) mod name_type;
pub(crate) mod peer_creds;

use crate::{local_socket::ListenerOptions, Sealed};
pub use name_type::*;

/// Unix-specific [listener options](ListenerOptions).
#[allow(private_bounds)]
pub trait ListenerOptionsExt: Sized + Sealed {
    /// Sets the file mode (Unix permissions) to be applied to the socket file. The default value
    /// is 777₈. The chosen mode will be affected by the process umask, which means that this
    /// option is not useful for making sockets accessible by any user. It is recommended to
    /// create those using the service manager, but setting the umask manually before creating
    /// the listener is also an option.
    ///
    /// The socket file mode authenticates clients using their process credentials according to
    /// the **write bits** of the mode; the read and execute bits are ignored by the OS and serve
    /// a cosmetic purpose.
    ///
    /// # Platform support
    /// This option is supported on the following platforms:
    /// - Linux
    /// - FreeBSD 14.3 (2025-06-10) and newer
    /// - OpenBSD
    ///
    /// On all other platforms, where this feature is not supported or has not been verified to be
    /// implemented correctly, [`Unsupported`] will be returned on listener creation. Please open
    /// an issue if you think an OS should be added to the list of supported ones.
    ///
    /// # Implementation notes
    /// An `fchmod` is performed on the socket prior to `bind` and `listen`. Previous versions of
    /// Interprocess made use of a racy fallback that modified the umask, but it did not receive
    /// adoption, and was removed in 2.3.0.
    ///
    /// illumos and Solaris cannot support this feature because `fchmod` before `bind` succeeds
    /// but does not affect the file mode of the resulting socket.
    ///
    /// [`Unsupported`]: std::io::ErrorKind::Unsupported
    #[must_use = builder_must_use!()]
    fn mode(self, mode: libc::mode_t) -> Self;
}

impl ListenerOptionsExt for ListenerOptions<'_> {
    #[inline(always)]
    fn mode(mut self, mode: libc::mode_t) -> Self {
        self.set_mode(mode);
        self
    }
}
