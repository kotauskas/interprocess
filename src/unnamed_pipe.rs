//! Creation and usage of unnamed pipes.
//!
//! The distinction between named and unnamed pipes is very clear given their names: named pipes have names in their
//! special named pipe filesystem, while unnamed pipes only have handles. This can both be useful or problematic,
//! depending on the use case. Unnamed pipes work best when a child process is used. With the fork model on Unix-like
//! systems, the handle can be transferred to the child process thanks to the cloned address space; on Windows,
//! inheritable handles (the default for unnamed pipes in this crate) can be used.
//!
//! Another way to use unnamed pipes is to use a named pipe or a Unix domain socket to establish an unnamed pipe
//! connection. It just so happens that this crate supports all three.

impmod! {unnamed_pipe,
    UnnamedPipeReader as UnnamedPipeReaderImpl,
    UnnamedPipeWriter as UnnamedPipeWriterImpl,
    pipe as pipe_impl,
}
use std::io;

/// Creates a new pipe with the default creation settings and returns the handles to its writing end and reading end.
///
/// The platform-specific builders in the `os` module of the crate might be more helpful if a configuration process for
/// the pipe is needed.
pub fn pipe() -> io::Result<(UnnamedPipeWriter, UnnamedPipeReader)> {
    pipe_impl()
}

/// A handle to the reading end of an unnamed pipe, created by the [`pipe()`] function together with the
/// [writing end](UnnamedPipeWriter).
///
/// The core functionality is exposed in a file-like [`Read`] interface. On Windows, the
/// [`ShareHandle`](crate::os::windows::ShareHandle) and [`As-`][ARH]/[`Into-`][IRH]/[`FromRawHandle`][FRH] traits are
/// also implemented, along with [`As-`][ARF]/[`Into-`][IRF]/[`FromRawFd`][FRF] on Unix.
///
/// [ARH]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html
/// [IRH]: https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawHandle.html
/// [FRH]: https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html
/// [ARF]: https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html
/// [IRF]: https://doc.rust-lang.org/std/os/unix/io/trait.IntoRawFd.html
/// [FRF]: https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html
// field is pub(crate) to allow the platform specific builders to create the public-facing pipe types
pub struct UnnamedPipeReader(pub(crate) UnnamedPipeReaderImpl);
multimacro! {
    UnnamedPipeReader,
    forward_sync_read,
    forward_handle,
    forward_try_clone,
    forward_debug,
    derive_raw,
}

/// A handle to the writing end of an unnamed pipe, created by the [`pipe()`] function together with the
/// [reading end](UnnamedPipeReader).
///
/// The core functionality is exposed in a file-like [`Write`] interface. On Windows, the
/// [`ShareHandle`](crate::os::windows::ShareHandle) and [`As-`][ARH]/[`Into-`][IRH]/[`FromRawHandle`][FRH] traits are
/// also implemented, along with [`As-`][ARF]/[`Into-`][IRF]/[`FromRawFd`][FRF] on Unix.
///
/// [AsRawHandle]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html
/// [IntoRawHandle]: https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawHandle.html
/// [FromRawHandle]: https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html
/// [AsRawFd]: https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html
/// [IntoRawFd]: https://doc.rust-lang.org/std/os/unix/io/trait.IntoRawFd.html
/// [FromRawFd]: https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html
pub struct UnnamedPipeWriter(pub(crate) UnnamedPipeWriterImpl);
multimacro! {
    UnnamedPipeWriter,
    forward_sync_write,
    forward_handle,
    forward_try_clone,
    forward_debug,
    derive_raw,
}
