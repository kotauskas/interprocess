//! Support for Unix domain sockets, abbreviated here as "Ud-sockets".
//!
//! Ud-sockets are a special kind of sockets which work in the scope of only one system and use file paths instead of
//! IPv4/IPv6 addresses and 16-bit socket numbers. Aside from their high reliability and convenience for the purposes of
//! IPC (such as filesystem-level privelege management and the similarity to named pipes), they have a unique feature
//! which cannot be replaced by any other form of IPC: **ancillary data**.
//!
//! # Ancillary data
//! Thanks to this feature, Ud-sockets can transfer ownership of a file descriptor to another process, even if it
//! doesn't have a parent-child relationship with the file descriptor owner and thus does not inherit anything via
//! `fork()`. Aside from that, ancillary data can contain credentials of a process, which are validated by the kernel
//! unless the sender is the superuser, meaning that this way of retrieving credentials can be used for
//! authentification.
//!
//! # Usage
//! The [`UdStreamListener`] and [`UdDatagram`] types are two starting points, depending on whether you intend to use
//! UDP-like datagrams or TCP-like byte streams.

pub mod cmsg;
#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

#[macro_use]
mod util;

mod datagram;
mod listener;
mod path;
mod socket_trait;
mod stream;

pub use {datagram::*, listener::*, path::*, socket_trait::*, stream::*};

#[cfg_attr(
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox",
        target_os = "freebsd",
        target_os = "dragonfly",
    )))
)]
#[cfg(any(uds_ucred, uds_cmsgcred))]
mod credentials;
#[cfg(any(uds_ucred, uds_cmsgcred))]
pub use credentials::*;

mod path_drop_guard;
use path_drop_guard::*;

mod c_wrappers;

/// The maximum path length for Unix domain sockets. [`UdStreamListener::bind()`] panics if the specified path exceeds
/// this value.
///
/// When using the [socket namespace](UdSocketPath::Namespaced), this value is reduced by 1, since enabling the usage of
/// that namespace takes up one character.
///
/// ## Value
/// The following platforms define the value of this constant as **108**:
/// - Linux
///     - includes Android
/// - Redox
///
/// The following platforms define the value of this constant as **104**:
/// - FreeBSD
/// - OpenBSD
/// - NetBSD
/// - DragonflyBSD
/// - macOS
/// - iOS
pub const MAX_UDSOCKET_PATH_LEN: usize = {
    use libc::{sa_family_t, sockaddr_un};
    use std::mem::size_of;

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
