//! Creating and using FIFO files, which are also known as "named pipes" but totally different from named pipes on Windows.
//!
//! On Windows, named pipes can be compared to Unix domain sockets: they can have multiple duplex connections on a single path, and the data can be chosen to either preserve or erase the message boundaries, resulting in a reliable performant implementation of TCP and UDP working in the bounds of a single machine. Those Unix domain sockets are also implemented by `interprocess` – see the [`udsocket`] module for that.
//!
//! On Linux, named pipes, referred to as "FIFO files" in this crate, are just files which can have a writer and a reader communicating with each other in one direction without message boundaries. If further readers try to open the file, they will simply read nothing at all; if further writers are connected, the data mixes in an unpredictable way, making it unusable. Therefore, FIFOs are to be used specifically to conveniently connect two applications through a known path which works like a pipe and nothing else.
//!
//! ## Usage
//! The [`create_fifo`] function serves for a FIFO file creation. Opening FIFO files works via the standard [`File`]s, opened either only for writing or only for reading. Deleting works the same way as with any regular file, via the [`remove_file`] function.
//!
//! [`udsocket`]: ../udsocket/index.html " "
//! [`create_fifo_file`]: fn.create_fifo.html " "
//! [`File`]: https://doc.rust-lang.org/stable/std/fs/struct.File.html " "
//! [`remove_file`]: https://doc.rust-lang.org/stable/std/fs/fn.remove_file.html " "

use std::{ffi::CString, io, path::Path};

use super::imports::*;

/// Creates a FIFO file at the specified path with the specified permissions.
///
/// Since the `mode` parameter is masked with the [`umask`], it's best to leave it at `0o777` unless a different value is desired.
///
/// ## System calls
/// - [`mkfifo`]
///
/// [`mkfifo`]: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mkfifo.html " "
/// [`umask`]: https://en.wikipedia.org/wiki/Umask " "
pub fn create_fifo<P: AsRef<Path>>(path: P, mode: mode_t) -> io::Result<()> {
    _create_fifo(path.as_ref(), mode)
}
fn _create_fifo(path: &Path, mode: mode_t) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    let success = unsafe { libc::mkfifo(path.as_bytes_with_nul().as_ptr() as *const _, mode) == 0 };
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
