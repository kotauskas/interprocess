mod enums;
mod error;
pub use {enums::*, error::*};

mod r#impl;
mod limbo;
mod wrapper_fns;
pub(super) use r#impl::*;
pub(crate) use wrapper_fns::*;

use super::{ConcurrencyDetector, MaybeArc, NeedsFlush};
use crate::os::windows::FileHandle;
use std::{marker::PhantomData, os::windows::prelude::*};

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
/// # Additional features
/// This section documents behavior introduced by this named pipe implementation which is not
/// present in the underlying Windows API.
///
/// ## Connection termination condition thunking
/// `ERROR_PIPE_NOT_CONNECTED` and [`BrokenPipe`](std::io::ErrorKind::BrokenPipe) errors are
/// translated to EOF (`Ok(0)`) for bytestreams and `RecvResult::EndOfStream` for message streams.
///
/// ## Flushing behavior
/// Upon being dropped, streams that haven't been flushed since the last send are transparently sent
/// to **limbo** – a thread pool that ensures that the peer does not get `BrokenPipe`/EOF
/// immediately after all data has been sent, which would otherwise discard everything. Named pipe
/// handles on this thread pool are flushed first and only then closed, ensuring that they are only
/// destroyed when the peer is done reading them.
///
/// If a stream hasn't seen a single send since the last explicit flush by the time it is dropped,
/// it will evade limbo. This can be overriden with [`.mark_dirty()`](PipeStream::mark_dirty).
///
/// Similarly to limbo elision, explicit flushes are elided on streams that haven't sent anything
/// since the last flush – thus, the second of any two consecutive `.flush()` calls is a no-op that
/// returns immediately and cannot fail. This can also be overridden in the same manner.
///
/// ## Concurrency prevention
/// Multiple I/O operations [cannot be performed on the same named pipe concurrently][ms], and
/// attempts to do so will be caught by the concurrency detector in order to avoid deadlocks and
/// other unexpected, chaotic behavior.
///
/// [ms]: https://learn.microsoft.com/en-nz/windows/win32/ipc/named-pipe-server-using-overlapped-i-o
///
/// # Examples
///
/// ## Basic bytestream client
/// ```no_run
/// use interprocess::os::windows::named_pipe::*;
/// use std::io::{BufReader, prelude::*};
///
/// // Preemptively allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let conn = DuplexPipeStream::<pipe_mode::Bytes>::connect("Example")?;
/// // Wrap it into a buffered reader right away so that we could receive a single line out of it.
/// let mut conn = BufReader::new(conn);
///
/// // Send our message into the stream. This will finish either when the whole message has been
/// // sent or if a send operation returns an error. (`.get_mut()` is to get the sender,
/// // `BufReader` doesn't implement a pass-through `Write`.)
/// conn.get_mut().write_all(b"Hello from client!\n")?;
///
/// // We now employ the buffer we allocated prior and receive a single line, interpreting a newline
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
/// // Preemptively allocate a sizeable buffer for receiving. Keep in mind that this will depend on
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
/// // Convert the data that's been received into a string. This checks for UTF-8
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

/// Type alias for a pipe stream with the same receive mode and send mode.
pub type DuplexPipeStream<M> = PipeStream<M, M>;

/// Type alias for a pipe stream with a receive mode but no send mode.
///
/// This can be produced by the listener, by connecting, or by splitting.
pub type RecvPipeStream<M> = PipeStream<M, pipe_mode::None>;
/// Type alias for a pipe stream with a send mode but no receive mode.
///
/// This can be produced by the listener, by connecting, or by splitting.
pub type SendPipeStream<M> = PipeStream<pipe_mode::None, M>;

pub(crate) struct RawPipeStream {
	handle: Option<FileHandle>,
	is_server: bool,
	needs_flush: NeedsFlush,
	concurrency_detector: ConcurrencyDetector,
}
