//! Support for named pipes on Windows.
//!
//! ## Windows named pipes are not Unix named pipes
//! The term "named pipe" refers to completely different things in Unix and Windows. For this reason, Unix named pipes
//! are referred to as "FIFO files" to avoid confusion with the more powerful Windows named pipes. In fact, the only
//! common features for those two is that they both can be located using filesystem paths and they both use a stream
//! interface. The differences can be summed up like this:
//! - Windows named pipes are located on a separate filesystem (NPFS – **N**amed **P**ipe **F**ile**s**ystem), while
//!   Unix FIFO files live in the shared filesystem tree together with all other files
//!     - On Linux, the implementation of Unix domain sockets exposes a similar feature: by setting the first byte in
//!       the socket file path to `NULL` (`\0`), the socket is placed into a separate namespace instead of being placed
//!       on the filesystem; this is a non-standard extension to POSIX and is not available on other Unix systems
//! - Windows named pipes have a server and an arbitrary number of clients, meaning that the separate processes
//!   connecting to a named pipe have separate connections to the server, while Unix FIFO files don't have the notion of
//!   a server or client and thus mix all data written into one sink from which the data is read by one process
//! - Windows named pipes can  be used over the network, while a Unix FIFO file is still local even if created in a
//!   directory which is a mounted network filesystem
//! - Windows named pipes can maintain datagram boundaries, allowing both sides of the connection to operate on separate
//!   messages rather than on a byte stream, while FIFO files, like any other type of file, expose only a byte stream
//!   interface
//!
//! If you carefully read through this list, you'd notice how Windows named pipes are similar to Unix domain sockets.
//! For this reason, the implementation of "local sockets" in the `local_socket` module of this crate uses named pipes
//! on Windows and Ud-sockets on Unix.

// TODO improve docs
// TODO add examples
// TODO document limbo
// TODO sync split
// TODO client impersonation

mod enums;
mod listener;
mod stream;
pub use {enums::*, listener::*, stream::*};

mod limbo_pool;
mod maybe_arc;
mod path_conversion;

#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

use super::winprelude::*;
use crate::os::windows::get_borrowed;
use std::{io, ptr};
use windows_sys::Win32::System::Pipes::SetNamedPipeHandleState;

unsafe fn set_nonblocking_for_stream(
    handle: BorrowedHandle<'_>,
    read_mode: Option<PipeMode>,
    nonblocking: bool,
) -> io::Result<()> {
    let read_mode: u32 = read_mode.map_or(0, PipeMode::to_readmode);
    // Bitcast the boolean without additional transformations since
    // the flag is in the first bit.
    let mut mode: u32 = read_mode | nonblocking as u32;
    let success = unsafe {
        SetNamedPipeHandleState(
            get_borrowed(handle),
            &mut mode as *mut _,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    } != 0;
    ok_or_ret_errno!(success => ())
}
