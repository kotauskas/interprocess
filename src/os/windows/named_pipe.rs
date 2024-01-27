//! Support for named pipes on Windows.
//!
//! # Those are not Unix named pipes
//! The term "named pipe" refers to completely different things in Unix and Windows. For this
//! reason, Unix named pipes are referred to as "FIFO files" to avoid confusion with the more
//! powerful Windows named pipes. In fact, the only common features for those two is that they both
//! can be located using filesystem paths and they both use a stream interface. The differences can
//! be summed up like this:
//! - Windows named pipes are located on a separate filesystem (NPFS – **N**amed **P**ipe
//!   **F**ile**s**ystem), while Unix FIFO files live in the shared filesystem tree together with
//!   all other files
//!     - On Linux, the implementation of Unix domain sockets exposes a similar feature: by setting
//!       the first byte in the socket file path to `NULL` (`\0`), the socket is placed into a
//!       separate namespace instead of being placed on the filesystem; this is a non-standard
//!       extension to POSIX and is not available on other Unix systems
//! - Windows named pipes have a server and an arbitrary number of clients, meaning that the
//!   separate processes connecting to a named pipe have separate connections to the server, while
//!   Unix FIFO files don't have the notion of a server or client and thus mix all data written
//!   into one sink from which the data is received by one process
//! - Windows named pipes can be used over the network, while a Unix FIFO file is still local even
//!   if created in a directory which is a mounted network filesystem
//! - Windows named pipes can maintain datagram boundaries, allowing both sides of the connection
//!   to operate on separate messages rather than on a byte stream, while FIFO files, like any
//!   other type of file, expose only a byte stream interface
//!
//! If you carefully read through this list, you'd notice how Windows named pipes are similar to
//! Unix domain sockets. For this reason, the implementation of "local sockets" in the
//! `local_socket` module of this crate uses named pipes on Windows and Unix-domain sockets on Unix.
//!
//! # Semantic peculiarities
//! Methods and I/O trait implementations on types presented in this module do not exactly map 1:1
//! to Windows API system calls. Below is a list of types with significant additional behavior.
//! - [`PipeStream`] and its async counterpart
// TODO make plural when introducing async-std
//!     - Conversion of [`BrokenPipe`](std::io::ErrorKind::BrokenPipe) reads to EOF (`Ok(0)`) for
//!       byte streams
//!         - Additionally, `ERROR_PIPE_NOT_CONNECTED` is converted to `BrokenPipe`
//!     - Limbo – transparent flush-on-close thread pool to ensure that the peer does not get a
//!       `BrokenPipe` (EOF if peer also uses Interprocess) immediately after the server is done
//!       sending data, which would discard everything
//!         - Limbo elision: any stream which, at the time of dropping, hasn't seen a single send
//!           since the last explicit flush, will evade limbo (can be overriden with
//!           [`.mark_dirty()`](PipeStream::mark_dirty))
//!     - Flush elision, analogous to limbo elision but also happens on explicit flush (i.e.
//!       flushing two times in a row only makes one system call)
//! - [`PipeListener`]
//!     - Is not a Win32-level concept, works by creating new instances right before returning from
//!       `.accept()`

// TODO improve docs
// TODO add examples
// TODO document limbo
// TODO client impersonation
// TODO raw instance functionality
// TODO transactions

mod enums;
mod listener;
mod stream;

/// Asynchronous named pipes which work with the Tokio runtime and event loop.
///
/// The Tokio integration allows the named pipe streams and listeners to be notified by the OS
/// kernel whenever they're ready to be received from or sent to, instead of spawning threads just
/// to put them in a wait state of blocking on the I/O.
///
/// Types from this module will *not* work with other async runtimes, such as `async-std` or `smol`,
/// since the Tokio types' methods will panic whenever they're called outside of a Tokio runtime
/// context. Open an issue if you'd like to see other runtimes supported as well.
#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio {
    mod listener;
    mod stream;
    pub use {listener::*, stream::*};
}

pub use {enums::*, listener::*, stream::*};

mod atomic_enum;
mod limbo_pool;
mod maybe_arc;
mod needs_flush;
mod path_conversion;

use {atomic_enum::*, maybe_arc::*, needs_flush::*};
