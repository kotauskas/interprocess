mod enums;
mod impls;
mod wrapper_fns;
pub(crate) use {enums::*, impls::*, wrapper_fns::*};

use crate::os::windows::FileHandle;
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    io,
    marker::PhantomData,
    sync::Arc,
};

pub(crate) static REUNITE_ERROR_MSG: &str = "the receive and self halves belong to different pipe stream objects";

/// A named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of receive modes and send modes, plugged into it using the `Rm` and `Sm` generic parameters respectively.
///
/// Pipe streams can be split by reference and by value for concurrent receive and send operations. Splitting by reference is ephemeral and can be achieved by simply borrowing the stream, since both `PipeStream` and `&PipeStream` implement I/O traits. Splitting by value is done using the [`.split()`](Self::split) method, producing a [`RecvHalf`] and a [`SendHalf`], and can be reverted via the `.reunite()` method defined on the halves.
// TODO examples
pub struct PipeStream<Rm: PipeModeTag, Sm: PipeModeTag> {
    raw: RawPipeStream,
    _phantom: PhantomData<(Rm, Sm)>,
}

/// Type alias for a pipe stream with the same read mode and write mode.
pub type DuplexPipeStream<M> = PipeStream<M, M>;

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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FromRawHandleErrorKind {
    /// It wasn't possible to determine whether the pipe handle corresponds to a pipe server or a pipe client.
    IsServerCheckFailed,
    /// The type being converted into has message semantics, but it wasn't possible to determine whether message boundaries are preserved in the pipe.
    MessageBoundariesCheckFailed,
    /// The type being converted into has message semantics, but message boundaries are not preserved in the pipe.
    NoMessageBoundaries,
}
/// Error type for `from_raw_handle()` constructors.
pub type FromRawHandleError = (FromRawHandleErrorKind, io::Error);

/// Result type for `.recv()` methods.
///
/// `Ok` indicates that the message fits in the provided buffer and was successfully received, `Err` indicates that it didn't fit and contains a new, bigger buffer which it was written to instead.
#[derive(Clone, Debug)]
pub enum RecvResult {
    Fit(usize),
    Alloc(Vec<u8>),
}
impl RecvResult {
    /// Returns the size of the message.
    #[inline]
    pub fn size(&self) -> usize {
        match self {
            Self::Fit(s) => *s,
            Self::Alloc(v) => v.len(),
        }
    }
    /// Returns whether the message was written to the buffer and taken off the OS queue or not
    #[inline]
    pub fn fit(&self) -> bool {
        matches!(self, Self::Fit(..))
    }
    /// Converts to a `Result<usize, Vec<u8>>`, where `Ok` represents `Fit` and `Err` represents `Alloc`.
    #[inline]
    pub fn into_result(self) -> Result<usize, Vec<u8>> {
        match self {
            Self::Fit(f) => Ok(f),
            Self::Alloc(a) => Err(a),
        }
    }
}
impl From<RecvResult> for Result<usize, Vec<u8>> {
    /// See `.into_result()`.
    fn from(x: RecvResult) -> Self {
        x.into_result()
    }
}

/// Result type for `.try_recv()` methods.
///
/// `Ok` indicates that the message fits in the provided buffer and was successfully received, `Err` indicates that it doesn't and hence wasn't written into the buffer. Both variants' payload is the total size of the message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TryRecvResult {
    /// The size of the message.
    pub size: usize,
    /// Whether the message was written to the buffer and taken off the OS queue or not.
    pub fit: bool,
}
impl TryRecvResult {
    /// Converts to a `Result<usize, usize>`, where `Ok` represents `fit = true` and `Err` represents `fit = false`.
    #[inline(always)]
    pub fn to_result(self) -> Result<usize, usize> {
        match (self.size, self.fit) {
            (s, true) => Ok(s),
            (s, false) => Err(s),
        }
    }
}
impl From<TryRecvResult> for Result<usize, usize> {
    /// See `.into_result()`.
    fn from(x: TryRecvResult) -> Self {
        x.to_result()
    }
}

/// Error type for `.reunite()` on split receive and send halves.
///
/// The error indicates that the halves belong to different streams and allows to recover both of them.
#[derive(Debug)]
pub struct ReuniteError<Rm: PipeModeTag, Sm: PipeModeTag> {
    pub recv_half: RecvHalf<Rm>,
    pub send_half: SendHalf<Sm>,
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Display for ReuniteError<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(REUNITE_ERROR_MSG)
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Error for ReuniteError<Rm, Sm> {}
