//! Creation and usage of unnamed pipes.
//!
//! The primary distinction between named and unnamed pipes is rather trivial: named pipes have
//! names in their special named pipe filesystem, while unnamed pipes only have handles. This can
//! both be useful or problematic, depending on the use case. Unnamed pipes work best when
//! communicating with child processes.
//!
//! The handles and file descriptors are inheritable by default. The `AsRawHandle` and `AsRawFd`
//! traits can be used to get a numeric handle value which can then be communicated to a child
//! process using a command-line argument, environment variable or some other program startup IPC
//! method. The numeric value can then be reconstructed into an I/O object using
//! `FromRawHandle`/`FromRawFd`. Interprocess does not concern itself with how this is done.
//!
//! Note
//! [the standard library's support for piping `stdin`, `stdout` and `stderr`](std::process::Stdio),
//! which can be used in simple cases instead. Making use of that feature is advisable if the
//! program of the child process can be modified to communicate with its parent via standard I/O
//! streams.

#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

impmod! {unnamed_pipe,
	Recver as RecverImpl,
	Sender as SenderImpl,
	pipe_impl,
}
use crate::Sealed;
use std::io;

/// Creates a new pipe with the default creation settings and returns the handles to its sending end
/// and receiving end.
///
/// The platform-specific builders in the `os` module of the crate might be more helpful if extra
/// configuration for the pipe is needed.
#[inline]
pub fn pipe() -> io::Result<(Sender, Recver)> {
	pipe_impl()
}

/// Handle to the receiving end of an unnamed pipe, created by the [`pipe()`] function together
/// with the [sending end](Sender).
///
/// The core functionality is exposed via the [`Read`](io::Read) trait. The type is convertible to
/// and from handles/file descriptors and allows its internal handle/FD to be borrowed. On
/// Windows, the `ShareHandle` trait is also implemented.
///
/// The handle/file descriptor is inheritable. See [module-level documentation](self) for more on
/// how this can be used.
// field is pub(crate) to allow platform builders to create the public-facing pipe types
pub struct Recver(pub(crate) RecverImpl);
impl Sealed for Recver {}
multimacro! {
	Recver,
	forward_sync_read,
	forward_handle,
	forward_debug,
	derive_raw,
}

/// Handle to the sending end of an unnamed pipe, created by the [`pipe()`] function together with
/// the [receiving end](Recver).
///
/// The core functionality is exposed via the [`Write`](io::Write) trait. The type is convertible
/// to and from handles/file descriptors and allows its internal handle/FD to be borrowed. On
/// Windows, the `ShareHandle` trait is also implemented.
///
/// The handle/file descriptor is inheritable. See [module-level documentation](self) for more on
/// how this can be used.
///
/// [ARH]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html
/// [IRH]: https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawHandle.html
/// [`FromRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html
/// [ARF]: https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html
/// [IRF]: https://doc.rust-lang.org/std/os/unix/io/trait.IntoRawFd.html
/// [`FromRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html
pub struct Sender(pub(crate) SenderImpl);
impl Sealed for Sender {}
multimacro! {
	Sender,
	forward_sync_write,
	forward_handle,
	forward_debug,
	derive_raw,
}
