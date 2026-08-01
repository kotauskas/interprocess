//! Creation of FIFO files.
//!
//! Those are sometimes referred to as named pipes. Note that they are completely unrelated to
//! the Windows concept with the same name. "FIFO files" are filesystem objects that allow for
//! synchronous arrangement of a unidirectional pipe connection between two processes, which is
//! only useful when one is not the ancestor of another and an unnamed pipe thus cannot be simply
//! inherited. This synchronization happens at file opening time and can be described as highly
//! aggressive: both the reader and the writer will block on opening the file until the other side
//! has also opened it.
//!
//! If multiple processes read from a FIFO file concurrently, they will compete for sent data; if
//! mulitple processes write to it concurrently, the data will mix unpredictably (albeit subject
//! to OS-specific thresholds of atomicity). In summary, concurrent use of a FIFO file by more
//! than two processes is almost always erroneous.
//!
//! Due to the above, use of FIFO files should be avoided if possible. You may be looking for
//! [local sockets](crate::local_socket) or [Unix domain sockets](std::os::unix::net) instead.
//!
//! ## Usage
//! The [`create_fifo()`] function serves for a FIFO file creation. Opening FIFO files works via the
//! standard [`File`](std::fs::File)s, opened either only for sending or only for receiving.
//! Deletion works the same way as with any regular file, via
//! [`remove_file()`](std::fs::remove_file).

use {
    super::unixprelude::*,
    crate::OrErrno,
    std::{ffi::CString, io, path::Path},
};

/// Creates a FIFO file at the specified path with the specified permissions.
///
/// Since the `mode` parameter is masked with the [`umask`], it's best to leave it at `0o777` unless
/// a different value is desired.
///
/// ## System calls
/// - [`mkfifo`]
///
/// [`mkfifo`]: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mkfifo.html
/// [`umask`]: https://en.wikipedia.org/wiki/Umask
pub fn create_fifo<P: AsRef<Path>>(path: P, mode: mode_t) -> io::Result<()> {
    _create_fifo(path.as_ref(), mode)
}
fn _create_fifo(path: &Path, mode: mode_t) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    unsafe { libc::mkfifo(path.as_bytes_with_nul().as_ptr().cast(), mode) != -1 }
        .true_val_or_errno(())
}
