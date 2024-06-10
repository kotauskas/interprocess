mod enums;
mod error;
pub use {enums::*, error::*};

mod r#impl;

use super::MaybeArc;
use crate::{
	local_socket::{ConcurrencyDetectionSite, ConcurrencyDetector},
	os::windows::{FileHandle, NeedsFlush},
};
use std::{marker::PhantomData, os::windows::prelude::*};

/// Named pipe stream, created by a server-side listener or by connecting to a server.
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
#[doc = doctest_file::include_doctest!("examples/named_pipe/sync/stream/bytes.rs")]
/// ```
///
/// ## Basic message stream client
/// ```no_run
#[doc = doctest_file::include_doctest!("examples/named_pipe/sync/stream/msg.rs")]
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
	concurrency_detector: ConcurrencyDetector<NamedPipeSite>,
}

#[derive(Default)]
struct NamedPipeSite;
impl ConcurrencyDetectionSite for NamedPipeSite {
	const NAME: &'static str = "named pipe";
	const WOULD_ACTUALLY_DEADLOCK: bool = true;
}
