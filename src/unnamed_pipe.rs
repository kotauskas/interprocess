//! Creation and usage of unnamed pipes.
//!
//! The distinction between named and unnamed pipes is concisely expressed by their names: where named pipes have names, unnamed pipes have handles. This can both be useful or problematic, depending on the use case. Unnamed pipes work best when a child process is used. With the fork model on Unix-like systems, the handle can be transferred to the child process thanks to the cloned address space; on Windows, inheritable handles can be used.
//!
//! Another way to use unnamed pipes is to use a named pipe or a Unix domain socket to establish an unnamed pipe connection. It just so happens that this crate supports all three.

impmod! {unnamed_pipe,
    UnnamedPipeReader as UnnamedPipeReaderImpl,
    UnnamedPipeWriter as UnnamedPipeWriterImpl,
    pipe as pipe_impl,
}
use std::{
    fmt::{self, Formatter},
    io::{self, Read, Write},
};

/// Creates a new pipe with the default creation settings and returns the handles to its writing end and reading end.
///
/// The platform-specific builders in the `os` module of the crate might be more helpful if a configuration process for the pipe is needed.
pub fn pipe() -> io::Result<(UnnamedPipeWriter, UnnamedPipeReader)> {
    pipe_impl()
}

/// A handle to the reading end of an unnamed pipe, created by the [`pipe`] function together with the [writing end].
///
/// The core functionality is exposed in a file-like [`Read`] interface. On Windows, the [`ShareHandle`] and [`As-`][`AsRawHandle`]/[`Into-`][`IntoRawHandle`]/[`FromRawHandle`] traits are also implemented, along with [`As-`][`AsRawFd`]/[`Into-`][`IntoRawFd`]/[`FromRawFd`] on Unix.
///
/// [`pipe`]: fn.pipe.html " "
/// [writing end]: struct.UnnamedPipeWriter.html " "
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html " "
/// [`ShareHandle`]: ../os/windows/trait.ShareHandle.html " "
/// [`AsRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html " "
/// [`IntoRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawHandle.html " "
/// [`FromRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html " "
/// [`AsRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html " "
/// [`IntoRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.IntoRawFd.html " "
/// [`FromRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html " "
pub struct UnnamedPipeReader {
    // pub(crate) to allow the platform specific builders to create the public-facing pipe types
    pub(crate) inner: UnnamedPipeReaderImpl,
}
impl Read for UnnamedPipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
impl fmt::Debug for UnnamedPipeReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
impl_handle_manip!(UnnamedPipeReader);

/// A handle to the writing end of an unnamed pipe, created by the [`pipe`] function together with the [reading end].
///
/// The core functionality is exposed in a file-like [`Write`] interface. On Windows, the [`ShareHandle`] and [`As-`][`AsRawHandle`]/[`Into-`][`IntoRawHandle`]/[`FromRawHandle`] traits are also implemented, along with [`As-`][`AsRawFd`]/[`Into-`][`IntoRawFd`]/[`FromRawFd`] on Unix.
///
/// [`pipe`]: fn.pipe.html " "
/// [reading end]: struct.UnnamedPipeReader.html " "
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html " "
/// [`ShareHandle`]: ../os/windows/trait.ShareHandle.html " "
/// [`AsRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html " "
/// [`IntoRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawHandle.html " "
/// [`FromRawHandle`]: https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html " "
/// [`AsRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html " "
/// [`IntoRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.IntoRawFd.html " "
/// [`FromRawFd`]: https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html " "
pub struct UnnamedPipeWriter {
    pub(crate) inner: UnnamedPipeWriterImpl,
}
impl Write for UnnamedPipeWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.inner.write(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl fmt::Debug for UnnamedPipeWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
impl_handle_manip!(UnnamedPipeWriter);
