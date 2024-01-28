// TODO message reading disabled due to a lack of support in Mio; we should try to figure something
// out, they need to add first-class message pipe support and handling of ERROR_MORE_DATA

mod error;
pub use error::*;

mod r#impl;
mod limbo;

use crate::os::windows::named_pipe::{
    stream::{pipe_mode, PipeModeTag},
    MaybeArc, NeedsFlush,
};
use std::{io, marker::PhantomData, sync::Mutex};
use tokio::net::windows::named_pipe::{
    NamedPipeClient as TokioNPClient, NamedPipeServer as TokioNPServer,
};

/// A Tokio-based named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of receive modes and send modes, plugged
/// into it using the `Rm` and `Sm` generic parameters respectively.
///
/// Pipe streams can be split by reference and by value for concurrent receive and send operations.
/// Splitting by reference is ephemeral and can be achieved by simply borrowing the stream, since
/// both `PipeStream` and `&PipeStream` implement the I/O traits. Splitting by value is done using
/// the [`.split()`](Self::split) method, producing a receive half and a send half, and can be
/// reverted via [`.reunite()`](Self::reunite).
///
/// # Examples
///
/// ## Basic bytestream client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use tokio::{io::{AsyncReadExt, AsyncWriteExt}, try_join};
/// use interprocess::os::windows::named_pipe::{pipe_mode, tokio::*};
///
/// // Await this here since we can't do a whole lot without a connection.
/// let conn = DuplexPipeStream::<pipe_mode::Bytes>::connect("Example").await?;
///
/// // This consumes our connection and splits it into two owned halves, so that we could
/// // concurrently act on both. Take care not to use the .split() method from the futures crate's
/// // AsyncReadExt.
/// let (mut recver, mut sender) = conn.split();
///
/// // Preemptively allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the send operation as sending our whole string, waiting for
/// // that to complete, and then shutting down the send half, which sends
/// // an EOF to the other end to help it determine where the message ends.
/// let send = async {
///     sender.write_all(b"Hello from client!").await?;
///     sender.shutdown().await?;
///     Ok(())
/// };
///
/// // Describe the receive operation as receiving until EOF into our big buffer.
/// let recv = recver.read_to_string(&mut buffer);
///
/// // Concurrently perform both operations: send-and-invoke-EOF and receive.
/// try_join!(send, recv)?;
///
/// // Get rid of those here to close the receive half too.
/// drop((recver, sender));
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

/// Type alias for a Tokio-based pipe stream with the same receive mode and send mode.
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
    inner: Option<InnerTokio>,
    // TODO crackhead specialization
    // Cleared by the generic pipes rather than by the raw pipe stream, unlike in sync land.
    needs_flush: NeedsFlush,
    // MESSAGE READING DISABLED
    //recv_msg_state: Mutex<RecvMsgState>,
}
// TODO maybe concurrency detection?
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
