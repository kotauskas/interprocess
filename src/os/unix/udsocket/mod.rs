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

pub mod cmsg;
#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

mod listener;
mod path;
mod socket;
mod stream;
mod util;
pub use {listener::*, path::*, socket::*, stream::*};

mod path_drop_guard;
use path_drop_guard::*;

mod c_wrappers;

use libc::{sa_family_t, sockaddr_un};
use std::mem::size_of;

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
pub const MAX_UDSOCKET_PATH_LEN: usize = {
    const LENGTH: usize = {
        let mut length = size_of::<sockaddr_un>() - size_of::<sa_family_t>();
        if cfg!(uds_sun_len) {
            length -= 1;
        }
        length
    };
    // Validates the calculated length and generates a cryptic compile error
    // if we guessed wrong, which isn't supposed to happen on any sane platform.
    let _ = sockaddr_un {
        #[cfg(uds_sun_len)]
        sun_len: 0,
        sun_family: 0,
        sun_path: [0; LENGTH],
    };
    LENGTH
};
