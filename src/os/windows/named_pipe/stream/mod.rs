mod enums;
pub use enums::*;

mod impls;
mod wrapper_fns;
pub(crate) use {impls::*, wrapper_fns::*};

use crate::os::windows::FileHandle;
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    io,
    marker::PhantomData,
    os::windows::prelude::*,
    sync::Arc,
};

pub(crate) static REUNITE_ERROR_MSG: &str = "the receive and self halves belong to different pipe stream objects";

/// A named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of receive modes and send modes, plugged into it using the `Rm` and `Sm` generic parameters respectively.
///
/// Pipe streams can be split by reference and by value for concurrent receive and send operations. Splitting by reference is ephemeral and can be achieved by simply borrowing the stream, since both `PipeStream` and `&PipeStream` implement I/O traits. Splitting by value is done using the [`.split()`](Self::split) method, producing a [`RecvHalf`] and a [`SendHalf`], and can be reverted via the `.reunite()` method defined on the halves.
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
/// use interprocess::{reliable_recv_msg::*, os::windows::named_pipe::*};
/// use std::io::{BufReader, prelude::*};
///
/// // Preemptively allocate a sizeable buffer for reading. Keep in mind that this will depend on
/// // the specifics of the protocol you're using.
/// let mut buffer = Vec::<u8>::with_capacity(128);
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
/// let rslt = conn.recv(&mut buffer)?;
///
/// // This borrows our message either from the new buffer or from the old one,
/// // cropped to its size. Note that this is one of `RecvResult`'s helpers.
/// let received_bytes = rslt.borrow_to_size(&buffer);
///
/// // Convert the data that's been read into a string. This checks for UTF-8
/// // validity, and if invalid characters are found, a new buffer is
/// // allocated to house a modified version of the received data, where
/// // decoding errors are replaced with those diamond-shaped question mark
/// // U+FFFD REPLACEMENT CHARACTER thingies: �.
/// let received_string = String::from_utf8_lossy(received_bytes);
///
/// // Print out the result!
/// println!("Server answered: {received_string}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct PipeStream<Rm: PipeModeTag, Sm: PipeModeTag> {
    raw: RawPipeStream,
    _phantom: PhantomData<(Rm, Sm)>,
}

/// Type alias for a pipe stream with the same read mode and write mode.
pub type DuplexPipeStream<M> = PipeStream<M, M>;

/// Type alias for a pipe stream with a read mode but no write mode.
pub type RecvPipeStream<M> = PipeStream<M, pipe_mode::None>;
/// Type alias for a pipe stream with a write mode but no read mode.
pub type SendPipeStream<M> = PipeStream<pipe_mode::None, M>;

/// The receiving half of a [`PipeStream`] as produced via `.split()`.
pub struct RecvHalf<Rm: PipeModeTag> {
    raw: Arc<RawPipeStream>,
    _phantom: PhantomData<Rm>,
}

/// The sending half of a [`PipeStream`] as produced via `.split()`.
pub struct SendHalf<Sm: PipeModeTag> {
    raw: Arc<RawPipeStream>,
    _phantom: PhantomData<Sm>,
}

pub(crate) struct RawPipeStream {
    pub(crate) handle: FileHandle,
    pub(crate) is_server: bool,
}

/// Additional contextual information for conversions from a raw handle to a named pipe stream.
///
/// Not to be confused with the Tokio version.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FromHandleErrorKind {
    /// It wasn't possible to determine whether the pipe handle corresponds to a pipe server or a pipe client.
    IsServerCheckFailed,
    /// The type being converted into has message semantics, but it wasn't possible to determine whether message boundaries are preserved in the pipe.
    MessageBoundariesCheckFailed,
    /// The type being converted into has message semantics, but message boundaries are not preserved in the pipe.
    NoMessageBoundaries,
}
impl FromHandleErrorKind {
    fn should_display_io_error(self) -> bool {
        !matches!(self, Self::NoMessageBoundaries)
    }
    const fn msg(self) -> &'static str {
        use FromHandleErrorKind::*;
        match self {
            IsServerCheckFailed => "failed to determine if the pipe is server-side or not",
            MessageBoundariesCheckFailed => "failed to make sure that the pipe preserves message boundaries",
            NoMessageBoundaries => "the pipe does not preserve message boundaries",
        }
    }
}
impl Display for FromHandleErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.msg())
    }
}

/// Error type for [`TryFrom<OwnedHandle>`](TryFrom) constructors.
///
/// Not to be confused with the Tokio version.
#[derive(Debug)]
pub struct FromHandleError {
    /// The stage at which the error occurred.
    pub kind: FromHandleErrorKind,
    /// The underlying OS error.
    pub io_error: io::Error,
    /// Ownership of the handle, so that it could be repurposed.
    pub handle: OwnedHandle,
}
impl Display for FromHandleError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        display_from_handle_error(
            f,
            self.kind.msg(),
            self.kind.should_display_io_error(),
            &self.io_error,
            self.handle.as_raw_handle(),
        )
    }
}
pub(crate) fn display_from_handle_error(
    f: &mut fmt::Formatter<'_>,
    kind: &'static str,
    should_display_io_error: bool,
    io_error: &io::Error,
    handle: RawHandle,
) -> fmt::Result {
    f.write_str(kind)?;
    if should_display_io_error {
        write!(f, ": {}", &io_error)?;
    }
    write!(f, " (handle: {handle:?})")
}

/// Error type for `.reunite()` on split receive and send halves.
///
/// The error indicates that the halves belong to different streams and allows to recover both of them.
#[derive(Debug)]
pub struct ReuniteError<Rm: PipeModeTag, Sm: PipeModeTag> {
    /// The receive half that didn't go anywhere, in case you still need it.
    pub recv_half: RecvHalf<Rm>,
    /// The send half that didn't go anywhere, in case you still need it.
    pub send_half: SendHalf<Sm>,
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Display for ReuniteError<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(REUNITE_ERROR_MSG)
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Error for ReuniteError<Rm, Sm> {}
