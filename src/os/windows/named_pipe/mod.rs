//! Support for named pipes on Windows.
//!
//! ## Windows named pipes are not Unix named pipes
//! The term "named pipe" refers to completely different things in Unix and Windows. For this reason, Unix named pipes are referred to as "FIFO files" to avoid confusion with the more powerful Windows named pipes. In fact, the only common features for those two is that they both can be located using filesystem paths and they both use a stream interface. The differences can be summed up like this:
//! - Windows named pipes are located on a separate filesystem (NPFS – **N**amed **P**ipe **F**ile**s**ystem), while Unix FIFO files live in the shared filesystem tree together with all other files
//!     - On Linux, the implementation of Unix domain sockets exposes a similar feature: by setting the first byte in the socket file path to `NULL` (`\0`), the socket is placed into a separate namespace instead of being placed on the filesystem; this is a non-standard extension to POSIX and is not available on other Unix systems
//! - Windows named pipes have a server and an arbitrary number of clients, meaning that the separate processes connecting to a named pipe have separate connections to the server, while Unix FIFO files don't have the notion of a server or client and thus mix all data written into one sink from which the data is read by one process
//! - Windows named pipes can be used over the network, while a Unix FIFO file is still local even if created in a directory which is a mounted network filesystem
//! - Windows named pipes can maintain datagram boundaries, allowing both sides of the connection to operate on separate messages rather than on a byte stream, while FIFO files, like any other type of file, expose only a byte stream interface
//!
//! If you carefully read through this list, you'd notice how Windows named pipes are similar to Unix domain sockets. For this reason, the implementation of "local sockets" in the `local_socket` module of this crate uses named pipes on Windows and Ud-sockets on Unix.

// TODO improve docs
// TODO add examples
// TODO get rid of the dumbass autoflush, literally no reason for me to have added it now that i
// actually write proper examples for this
// FIXME message streams should have methods instead of I/O traits

mod enums;
mod listener;
mod new_stream;
mod pipeops;
#[macro_use]
mod stream;
use pipeops::*;
pub use {enums::*, listener::*, stream::*};

#[cfg(any(doc, feature = "tokio_support"))]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio_support")))]
#[cfg_attr(not(feature = "tokio_support"), allow(unused_imports))]
pub mod tokio;

use super::imports::*;
use std::{
    ffi::{OsStr, OsString},
    io, iter, ptr,
};

fn pathcvt<'a>(
    pipe_name: &'a OsStr,
    hostname: Option<&'a OsStr>,
) -> (impl Iterator<Item = &'a OsStr>, usize) {
    use iter::once as i;

    static PREFIX_LITERAL: &str = r"\\";
    static PIPEFS_LITERAL: &str = r"\pipe\";
    static LOCAL_HOSTNAME: &str = ".";

    let hostname = hostname.unwrap_or_else(|| OsStr::new(LOCAL_HOSTNAME));

    let iterator = i(OsStr::new(PREFIX_LITERAL))
        .chain(i(hostname))
        .chain(i(OsStr::new(PIPEFS_LITERAL)))
        .chain(i(pipe_name));
    let capacity_hint =
        PREFIX_LITERAL.len() + hostname.len() + PIPEFS_LITERAL.len() + pipe_name.len();
    (iterator, capacity_hint)
}
fn convert_path(pipename: &OsStr, hostname: Option<&OsStr>) -> OsString {
    let (i, cap) = pathcvt(pipename, hostname);
    let mut path = OsString::with_capacity(cap);
    i.for_each(|c| path.push(c));
    path
}
fn convert_and_encode_path(pipename: &OsStr, hostname: Option<&OsStr>) -> Vec<u16> {
    let (i, cap) = pathcvt(pipename, hostname);
    let mut path = Vec::with_capacity(cap + 1);
    i.for_each(|c| path.extend(c.encode_wide()));
    path.push(0); // Don't forget the nul terminator!
    path
}
fn encode_to_utf16(s: &OsStr) -> Vec<u16> {
    let mut path = s.encode_wide().collect::<Vec<u16>>();
    path.push(0);
    path
}
#[cfg(windows)]
unsafe fn set_nonblocking_for_stream(
    handle: HANDLE,
    read_mode: Option<PipeMode>,
    nonblocking: bool,
) -> io::Result<()> {
    let read_mode: u32 = read_mode.map_or(0, PipeMode::to_readmode);
    // Bitcast the boolean without additional transformations since
    // the flag is in the first bit.
    let mut mode: u32 = read_mode | nonblocking as u32;
    let success = unsafe {
        SetNamedPipeHandleState(
            handle,
            &mut mode as *mut _,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    } != 0;
    ok_or_ret_errno!(success => ())
}
