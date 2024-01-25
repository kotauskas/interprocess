mod enums;
pub use enums::*;

mod r#impl;
mod limbo;
mod wrapper_fns;
pub(super) use r#impl::*;
pub(crate) use wrapper_fns::*;

use super::{MaybeArc, NeedsFlush};
use crate::{error::ConversionError, os::windows::FileHandle};
use std::{
    fmt::{self, Debug, Display, Formatter},
    io,
    marker::PhantomData,
    os::windows::prelude::*,
};

/// A named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of
/// [receive modes and send modes](pipe_mode), plugged into it using the `Rm` and `Sm` generic
/// parameters respectively.
///
/// Pipe streams can be split by reference and by value for concurrent receive and send operations.
/// Splitting by reference is ephemeral and can be achieved by simply borrowing the stream, since
/// both `PipeStream` and `&PipeStream` implement I/O traits. Splitting by value is done using the
/// [`.split()`](Self::split) method, producing a receive half and a send half, and can be reverted
/// via [`.reunite()`](PipeStream::reunite).
///
/// # Semantic peculiarities
/// - [`BrokenPipe`](io::ErrorKind::BrokenPipe) errors from read methods are converted to EOF
///   (`Ok(0)`)
/// - Upon drop, streams that haven't been flushed since the last write are transparently sent to
///   **limbo** – a thread pool that ensures that the peer does not get a `BrokenPipe` (EOF if peer
///   also uses Interprocess) immediately after the server is done sending data, which would discard
///   everything
///     - At the time of dropping, if the stream hasn't seen a single write since the last explicit
///       flush, it will evade limbo (can be overriden with
///       [`.mark_dirty()`](PipeStream::mark_dirty))
/// - Flush elision, analogous to limbo elision but also happens on explicit flush (i.e. flushing
///   two times in a row only makes one system call)
///
/// # Examples
///
/// ## Basic bytestream client
/// ```no_run
/// use interprocess::os::windows::named_pipe::*;
/// use std::io::{BufReader, prelude::*};
///
/// // Preemptively allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let conn = DuplexPipeStream::<pipe_mode::Bytes>::connect("Example")?;
/// // Wrap it into a buffered reader right away so that we could read a single line out of it.
/// let mut conn = BufReader::new(conn);
///
/// // Write our message into the stream. This will finish either when the whole message has been
/// // writen or if a write operation returns an error. (`.get_mut()` is to get the writer,
/// // `BufReader` doesn't implement a pass-through `Write`.)
/// conn.get_mut().write_all(b"Hello from client!\n")?;
///
/// // We now employ the buffer we allocated prior and read a single line, interpreting a newline
/// // character as an end-of-file (because local sockets cannot be portably shut down), verifying
/// // validity of UTF-8 on the fly.
/// conn.read_line(&mut buffer)?;
///
/// // Print out the result, getting the newline for free!
/// print!("Server answered: {buffer}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## Basic message stream client
/// ```no_run
/// use recvmsg::prelude::*;
/// use interprocess::os::windows::named_pipe::*;
///
/// // Preemptively allocate a sizeable buffer for reading. Keep in mind that this will depend on
/// // the specifics of the protocol you're using.
/// let mut buffer = MsgBuf::from(Vec::with_capacity(128));
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let mut conn = DuplexPipeStream::<pipe_mode::Messages>::connect("Example")?;
///
/// // Here's our message so that we could check its length later.
/// static MESSAGE: &[u8] = b"Hello from client!";
/// // Send the message, getting the amount of bytes that was actually sent in return.
/// let sent = conn.send(MESSAGE)?;
/// assert_eq!(sent, MESSAGE.len()); // If it doesn't match, something's seriously wrong.
///
/// // Use the reliable message receive API, which gets us a `RecvResult` from the
/// // `reliable_recv_msg` module.
/// conn.recv_msg(&mut buffer, None)?;
///
/// // Convert the data that's been read into a string. This checks for UTF-8
/// // validity, and if invalid characters are found, a new buffer is
/// // allocated to house a modified version of the received data, where
/// // decoding errors are replaced with those diamond-shaped question mark
/// // U+FFFD REPLACEMENT CHARACTER thingies: �.
/// let received_string = String::from_utf8_lossy(buffer.filled_part());
///
/// // Print out the result!
/// println!("Server answered: {received_string}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct PipeStream<Rm: PipeModeTag, Sm: PipeModeTag> {
    raw: MaybeArc<RawPipeStream>,
    _phantom: PhantomData<(Rm, Sm)>,
}

/// Type alias for a pipe stream with the same read mode and write mode.
pub type DuplexPipeStream<M> = PipeStream<M, M>;

/// Type alias for a pipe stream with a read mode but no write mode.
///
/// This can be produced by the listener, by connecting, or by splitting.
pub type RecvPipeStream<M> = PipeStream<M, pipe_mode::None>;
/// Type alias for a pipe stream with a write mode but no read mode.
///
/// This can be produced by the listener, by connecting, or by splitting.
pub type SendPipeStream<M> = PipeStream<pipe_mode::None, M>;

pub(crate) struct RawPipeStream {
    handle: Option<FileHandle>,
    is_server: bool,
    needs_flush: NeedsFlush,
}

/// Additional contextual information for conversions from a raw handle to a named pipe stream.
///
/// Not to be confused with the Tokio version.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FromHandleErrorKind {
    /// It wasn't possible to determine whether the pipe handle corresponds to a pipe server or a pipe client.
    IsServerCheckFailed,
    /// The type being converted into has message semantics, but it wasn't possible to determine whether message
    /// boundaries are preserved in the pipe.
    MessageBoundariesCheckFailed,
    /// The type being converted into has message semantics, but message boundaries are not preserved in the pipe.
    NoMessageBoundaries,
}
impl FromHandleErrorKind {
    const fn msg(self) -> &'static str {
        use FromHandleErrorKind::*;
        match self {
            IsServerCheckFailed => "failed to determine if the pipe is server-side or not",
            MessageBoundariesCheckFailed => "failed to make sure that the pipe preserves message boundaries",
            NoMessageBoundaries => "the pipe does not preserve message boundaries",
        }
    }
}
impl From<FromHandleErrorKind> for io::Error {
    fn from(e: FromHandleErrorKind) -> Self {
        io::Error::new(io::ErrorKind::Other, e.msg())
    }
}
impl Display for FromHandleErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.msg())
    }
}

/// Error type for [`TryFrom<OwnedHandle>`](TryFrom) constructors.
///
/// Not to be confused with the Tokio version.
pub type FromHandleError = ConversionError<OwnedHandle, FromHandleErrorKind>;

/// [`ReuniteError`](crate::error::ReuniteError) for sync named pipe streams.
pub type ReuniteError<Rm, Sm> = crate::error::ReuniteError<RecvPipeStream<Rm>, SendPipeStream<Sm>>;

/// Result type for [`PipeStream::reunite()`].
pub type ReuniteResult<Rm, Sm> = Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>>;
