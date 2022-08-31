//! Support for Unix domain sockets, abbreviated here as "Ud-sockets".
//!
//! Ud-sockets are a special kind of sockets which work in the scope of only one system and use file paths instead of IPv4/IPv6 addresses and 16-bit socket numbers. Aside from their high reliability and convenience for the purposes of IPC (such as filesystem-level privelege management and the similarity to named pipes), they have a unique feature which cannot be replaced by any other form of IPC: **ancillary data**.
//!
//! # Ancillary data
//! Thanks to this feature, Ud-sockets can transfer ownership of a file descriptor to another process, even if it doesn't have a parent-child relationship with the file descriptor owner and thus does not inherit anything via `fork()`. Aside from that, ancillary data can contain credentials of a process, which are validated by the kernel unless the sender is the superuser, meaning that this way of retrieving credentials can be used for authentification.
//!
//! # Usage
//! The [`UdStreamListener`] and [`UdSocket`] types are two starting points, depending on whether you intend to use UDP-like datagrams or TCP-like byte streams.
//!
//! [`UdStreamListener`]: struct.UdStreamListener.html " "
//! [`UdSocket`]: struct.UdSocket.html " "

#[cfg(any(doc, feature = "tokio_support"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio_support")))]
pub mod tokio;

mod ancillary;
mod listener;
mod path;
mod socket;
mod stream;
mod util;
pub use {ancillary::*, listener::*, path::*, socket::*, stream::*};

mod path_drop_guard;
use path_drop_guard::*;

#[cfg(uds_supported)]
mod c_wrappers;

use super::imports;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(not(unix))] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 0;
    } else if #[cfg(uds_sockaddr_un_len_108)] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 108;
    } else if #[cfg(uds_sockaddr_un_len_104)] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 104;
    } else if #[cfg(uds_sockaddr_un_len_126)] {
        const _MAX_UDSOCKET_PATH_LEN: usize = 126;
    } else {
        compile_error!("\
Please fill out MAX_UDSOCKET_PATH_LEN in interprocess/src/os/unix/udsocket/mod.rs for your \
platform if you wish to enable Unix domain socket support for it"
        );
    }
}

/// The maximum path length for Unix domain sockets. [`UdStreamListener::bind`] panics if the specified path exceeds this value.
///
/// When using the [socket namespace], this value is reduced by 1, since enabling the usage of that namespace takes up one character.
///
/// ## Value
/// The following platforms define the value of this constant as **108**:
/// - Linux
///     - includes Android
/// - Emscripten
/// - Redox
/// - HermitCore
/// - Solaris
///     - Illumos
///
/// The following platforms define the value of this constant as **104**:
/// - FreeBSD
/// - OpenBSD
/// - NetBSD
/// - DragonflyBSD
/// - macOS
/// - iOS
///
/// The following platforms define the value of this constant as **126**:
/// - Haiku
///
/// [`UdStreamListener::bind`]: struct.UdStreamListener.html#method.bind " "
/// [socket namespace]: enum.UdSocketPath.html#namespaced " "
// The reason why this constant wraps the underscored one instead of being defined directly is
// because that'd require documenting both branches separately. This way, the user-faced
// constant has only one definition and one documentation comment block.
pub const MAX_UDSOCKET_PATH_LEN: usize = _MAX_UDSOCKET_PATH_LEN;
