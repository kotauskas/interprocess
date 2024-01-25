// TODO message reading disabled due to a lack of support in Mio; we should try to figure something
// out, they need to add first-class message pipe support and handling of ERROR_MORE_DATA
mod r#impl;
mod limbo;

use crate::{
    error::ConversionError,
    os::windows::{
        named_pipe::{
            stream::{pipe_mode, PipeModeTag},
            MaybeArc, NeedsFlush,
        },
        winprelude::*,
    },
};
use std::{
    fmt::{self, Display, Formatter},
    io,
    marker::PhantomData,
    sync::Mutex,
};
use tokio::net::windows::named_pipe::{NamedPipeClient as TokioNPClient, NamedPipeServer as TokioNPServer};

/// A Tokio-based named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of receive modes and send modes, plugged into it using the
/// `Rm` and `Sm` generic parameters respectively.
///
/// Pipe streams can be split by reference and by value for concurrent receive and send operations. Splitting by
/// reference is ephemeral and can be achieved by simply borrowing the stream, since both `PipeStream` and `&PipeStream`
/// implement the I/O traits. Splitting by value is done using the [`.split()`](Self::split) method, producing a
/// receive half and a send half, and can be reverted via [`.reunite()`](Self::reunite).
///
/// # Examples
///
/// ## Basic bytestream client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufReader}, try_join};
/// use interprocess::os::windows::named_pipe::{pipe_mode, tokio::*};
///
/// // Await this here since we can't do a whole lot without a connection.
/// let conn = DuplexPipeStream::<pipe_mode::Bytes>::connect("Example").await?;
///
/// // This consumes our connection and splits it into two owned halves, so that we could
/// // concurrently act on both. Take care not to use the .split() method from the futures crate's
/// // AsyncReadExt.
/// let (mut reader, mut writer) = conn.split();
///
/// // Preemptively allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the write operation as writing our whole string, waiting for
/// // that to complete, and then shutting down the write half, which sends
/// // an EOF to the other end to help it determine where the message ends.
/// let write = async {
///     writer.write_all(b"Hello from client!").await?;
///     writer.shutdown().await?;
///     Ok(())
/// };
///
/// // Describe the read operation as reading until EOF into our big buffer.
/// let read = reader.read_to_string(&mut buffer);
///
/// // Concurrently perform both operations: write-and-send-EOF and read.
/// try_join!(write, read)?;
///
/// // Get rid of those here to close the read half too.
/// drop((reader, writer));
///
/// // Display the results when we're done!
/// println!("Server answered: {}", buffer.trim());
/// # Ok(()) }
/// ```
pub struct PipeStream<Rm: PipeModeTag, Sm: PipeModeTag> {
    raw: MaybeArc<RawPipeStream>,
    flush: Mutex<Option<FlushJH>>,
    _phantom: PhantomData<(Rm, Sm)>,
}
type FlushJH = tokio::task::JoinHandle<io::Result<()>>;

/// Type alias for a Tokio-based pipe stream with the same read mode and write mode.
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
    inner: Option<InnerTokio>,
    // TODO crackhead specialization
    // Cleared by the generic pipes rather than the raw pipe stream unlike in sync land.
    needs_flush: NeedsFlush,
    // MESSAGE READING DISABLED
    //recv_msg_state: Mutex<RecvMsgState>,
}
enum InnerTokio {
    Server(TokioNPServer),
    Client(TokioNPClient),
}

/* MESSAGE READING DISABLED
#[derive(Debug, Default)]
#[repr(u8)]
enum RecvMsgState {
    #[default]
    NotRecving,
    Looping {
        spilled: bool,
        partial: bool,
    },
    Discarding {
        result: io::Result<RecvResult>,
    },
}
unsafe impl ReprU8 for RecvMsgState {}
*/

/// Additional contextual information for conversions from a raw handle to a named pipe stream.
///
/// Not to be confused with the [non-Tokio version](crate::os::windows::named_pipe::stream::FromHandleErrorKind).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FromHandleErrorKind {
    /// It wasn't possible to determine whether the pipe handle corresponds to a pipe server or a pipe client.
    IsServerCheckFailed,
    /// The type being converted into has message semantics, but it wasn't possible to determine whether message
    /// boundaries are preserved in the pipe.
    MessageBoundariesCheckFailed,
    /// The type being converted into has message semantics, but message boundaries are not preserved in the pipe.
    NoMessageBoundaries,
    /// An error was reported by Tokio.
    ///
    /// Most of the time, this means that `from_raw_handle()` call was performed outside of the Tokio runtime, but OS
    /// errors associated with the registration of the handle in the runtime belong to this category as well.
    TokioError,
}
impl FromHandleErrorKind {
    const fn msg(self) -> &'static str {
        use FromHandleErrorKind::*;
        match self {
            IsServerCheckFailed => "failed to determine if the pipe is server-side or not",
            MessageBoundariesCheckFailed => "failed to make sure that the pipe preserves message boundaries",
            NoMessageBoundaries => "the pipe does not preserve message boundaries",
            TokioError => "Tokio error",
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
/// Not to be confused with the [non-Tokio version](crate::os::windows::named_pipe::stream::FromHandleError).
pub type FromHandleError = ConversionError<OwnedHandle, FromHandleErrorKind>;

/// [`ReuniteError`](crate::error::ReuniteError) for Tokio named pipe streams.
pub type ReuniteError<Rm, Sm> = crate::error::ReuniteError<RecvPipeStream<Rm>, SendPipeStream<Sm>>;

/// Result type for [`PipeStream::reunite()`].
pub type ReuniteResult<Rm, Sm> = Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>>;
