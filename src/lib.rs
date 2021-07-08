//! Interprocess communication toolkit for Rust programs. The crate aims to expose as many platform-specific features as possible while maintaining a uniform interface for all platforms.
//!
//! # Features
//! The following interprocess communication primitives are implemented:
//! - **Unnamed pipes** — anonymous file-like objects for communicating privately in one direction, most commonly used to communicate between a child process and its parent
//! - **FIFO files** — Unix-specific type of file which is similar to unnamed pipes but exists on the filesystem, often referred to as "named pipes" but completely different from Windows named pipes
//! - **Unix domain sockets** — Unix-specific socket type which is extremely similar to normal network sockets but uses filesystem paths instead, with the optional Linux feature allowing them to use a spearate namespace akin to Windows named pipes
//! - **Windows named pipes** — Windows-specific named pipe interface closely resembling Unix domain sockets
//! - **Local sockets** — platform independent interface utilizing named pipes on Windows and Unix domain sockets on Unix. **Async support included!**
//! - **Signals** — Unix-specific signals, used to receive critical messages from the OS and other programs, as well as sending those messages
//!
//! # License
//! This crate, along with all community contributions made to it, is dual-licensed under the terms of either the [MIT license] or the [Apache 2.0 license].
//!
//! [MIT license]: https://choosealicense.com/licenses/mit/ " "
//! [Apache 2.0 license]: https://choosealicense.com/licenses/apache-2.0/ " "
// TODO mailslots
// TODO shared memory
// - **Mailslots** — Windows-specific interprocess communication primitive for short messages, potentially even across the network
// - **Shared memory** — exposes a nice safe interface for shared memory based on mapping identifiers, with some additional platform-specific extensions

#![cfg_attr(feature = "doc_cfg", feature(doc_cfg))]
#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![allow(clippy::nonstandard_macro_braces)]

// If an operating system is not listed here, the `compile_error!` is invoked
#[cfg(not(any(
    // "Linux-like" (src/unix/linux_like/mod.rs in libc)
    target_os = "linux",
    target_os = "android",
    target_os = "emscripten",

    // Windows. There is just one.
    target_os = "windows",

    // "BSD-like" (src/unix/bsd/mod.rs in libc)
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",

    // "Solarish" (src/unix/solarish/mod.rs in libc)
    target_os = "solaris",
    target_os = "illumos",

    // Haiku (src/unix/haiku/mod.rs in libc)
    target_os = "haiku",

    // Hermit (src/unix/hermit/mod.rs in libc)
    target_os = "hermit",

    // Redox (src/unix/redox/mod.rs in libc)
    target_os = "redox",
)))]
compile_error!("Your target operating system is not supported by interprocess — check if yours is in the list of supported systems, and if not, please open an issue on the GitHub repository if you think that it should be included");

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("Platforms with exotic pointer widths (neither 32-bit nor 64-bit) are not supported by interprocess — if you think that your specific case needs to be accounted for, please open an issue on the GitHub repository");

pub(crate) use private::Sealed;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
};
#[macro_use]
pub(crate) mod private {
    macro_rules! impmod {
        ($osmod:ident, $($orig:ident $(as $into:ident)?),* $(,)?) => {
            #[cfg(unix)]
            use $crate::os::unix::$osmod::{$($orig $(as $into)?,)*};
            #[cfg(windows)]
            use $crate::os::windows::$osmod::{$($orig $(as $into)?,)*};
        };
    }
    macro_rules! impl_as_raw_handle {
        ($ty:ident) => {
            #[cfg(windows)]
            impl ::std::os::windows::io::AsRawHandle for $ty {
                #[inline]
                fn as_raw_handle(&self) -> *mut ::std::ffi::c_void {
                    ::std::os::windows::io::AsRawHandle::as_raw_handle(&self.inner)
                }
            }
            #[cfg(unix)]
            impl ::std::os::unix::io::AsRawFd for $ty {
                #[inline]
                fn as_raw_fd(&self) -> ::libc::c_int {
                    ::std::os::unix::io::AsRawFd::as_raw_fd(&self.inner)
                }
            }
        };
    }
    macro_rules! impl_into_raw_handle {
        ($ty:ident) => {
            #[cfg(windows)]
            impl ::std::os::windows::io::IntoRawHandle for $ty {
                #[inline]
                fn into_raw_handle(self) -> *mut ::std::ffi::c_void {
                    ::std::os::windows::io::IntoRawHandle::into_raw_handle(self.inner)
                }
            }
            #[cfg(unix)]
            impl ::std::os::unix::io::IntoRawFd for $ty {
                #[inline]
                fn into_raw_fd(self) -> ::libc::c_int {
                    ::std::os::unix::io::IntoRawFd::into_raw_fd(self.inner)
                }
            }
        };
    }
    macro_rules! impl_from_raw_handle {
        ($ty:ident) => {
            #[cfg(windows)]
            impl ::std::os::windows::io::FromRawHandle for $ty {
                #[inline]
                unsafe fn from_raw_handle(handle: *mut ::std::ffi::c_void) -> Self {
                    Self {
                        inner: unsafe {
                            ::std::os::windows::io::FromRawHandle::from_raw_handle(handle)
                        },
                    }
                }
            }
            #[cfg(unix)]
            impl ::std::os::unix::io::FromRawFd for $ty {
                #[inline]
                unsafe fn from_raw_fd(fd: ::libc::c_int) -> Self {
                    Self {
                        inner: unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) },
                    }
                }
            }
        };
    }
    macro_rules! impl_handle_manip {
        ($ty:ident) => {
            impl_as_raw_handle!($ty);
            impl_into_raw_handle!($ty);
            impl_from_raw_handle!($ty);
        };
    }
    // If the trait itself was pub(crate), it wouldn't work as a supertrait on public traits. We use a
    // private module instead to make it impossible to name the trait from outside the crate.
    pub trait Sealed {}
}

pub mod local_socket;
#[cfg(any(doc, feature = "nonblocking"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "nonblocking")))]
#[deprecated(note = "\
does not integrate with async runtimes, leading to poor performance and bugs related to reading \
and writing at the same time (you can't) — see the `tokio` modules for relevant IPC primitives \
or open an issue if you want more async runtimes to be supported as well")]
pub mod nonblocking;
pub mod unnamed_pipe;
//pub mod shared_memory;

pub mod os;

/// Reading from named pipes with message boundaries reliably, without truncation.
///
/// ## The problem
/// Unlike a byte stream interface, message-mode named pipes preserve boundaries between different write calls, which is what "message boundary" essentially means. Extracting messages by partial reads is an error-prone task, which is why no such interface is exposed by the operating system — instead, all messages read from a named pipe stream are full messages rather than chunks of messages, which simplifies things to a great degree and is arguably the only proper way of implementing datagram support.
///
/// There is one pecularity related to this design: you can't just use a buffer with arbitrary length to successfully read a message. With byte streams, that always works — there either is some data which can be written into that buffer or end of file has been reached, aside from the implied error case which is always a possibility for any kind of I/O. With message streams, however, **there might not always be enough space in a buffer to fetch a whole message**. If the buffer is too small to fetch a message, it won't be written into the buffer, but simply will be ***discarded*** instead. The only way to protect from it being discarded is first checking whether the message fits into the buffer without discarding it and then actually reading it into a suitably large buffer. In such a case, the message needs an alternate channel besides the buffer to somehow get returned.
///
/// This brings the discussion specifically to the signature of the `read_msg` method:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>>;
/// # }
/// ```
/// Setting aside from the `io::Result` part, the "true return value" is `Result<usize, Vec<u8>>`. The `Ok(...)` variant here means that the message has been successfully read into the buffer and contains the actual size of the message which has been read. The `Err(...)` variant means that the buffer was too small for the message, containing a freshly allocated buffer which is just big enough to fit the message. The usage strategy is to store a buffer, mutably borrow it and pass it to the `read_msg` function, see if it fits inside the buffer, and if it does not, replace the stored buffer with the new one.
///
/// The `try_read_msg` method is a convenience function used mainly by implementations of `read_msg` to determine whether it's required to allocate a new buffer or not. It has the following signature:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>>;
/// # }
/// ```
/// While it may seem strange how the nested `Result` returns the same type in `Ok` and `Err`, it does this for a semantic reason: the `Ok` variant means that the message was successfully read into the buffer while `Err` means the opposite — that the message was too big — and returns the size which the buffer needs to have.
///
/// ## Platform support
/// The trait is implemented for:
/// - Named pipes on Windows (module `interprocess::os::windows::named_pipe`)
/// - Unix domain pipes, but only on Linux (module `interprocess::os::unix::udsocket`)
///     - This is because only Linux provides a special flag for `recv` which returns the amount of bytes in the message regardless of the provided buffer size when peeking.
pub trait ReliableReadMsg: Sealed {
    /// Reads one message from the stream into the specified buffer, returning either the size of the message written, a bigger buffer if the one provided was too small, or an error in the outermost `Result` if the operation could not be completed for OS reasons.
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>>;

    /// Attempts to read one message from the stream into the specified buffer, returning the size of the message, which, depending on whether it was in the `Ok` or `Err` variant, either did fit or did not fit into the provided buffer, respectively; if the operation could not be completed for OS reasons, an error from the outermost `Result` is returned.
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>>;
}

/// Marker error indicating that a datagram write operation failed because the amount of bytes which were actually written as reported by the operating system was smaller than the size of the message which was requested to be written.
///
/// Always emitted with the `ErrorKind::Other` error type.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialMsgWriteError;
impl Display for PartialMsgWriteError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("message write operation wrote less than the size of the message")
    }
}
impl Error for PartialMsgWriteError {}
