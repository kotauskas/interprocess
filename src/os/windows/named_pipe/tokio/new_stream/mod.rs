mod impls;
mod wrapper_fns;
pub(crate) use wrapper_fns::*;

use super::{
    super::new_stream::{pipe_mode, PipeModeTag, RecvResult, TryRecvResult},
    imports::*,
};
use std::{io, marker::PhantomData};

/// A Tokio-based named pipe stream, created by a server-side listener or by connecting to a server.
///
/// This type combines in itself all possible combinations of receive modes and send modes, plugged into it using the `Rm` and `Sm` generic parameters respectively.
// TODO examples
pub struct PipeStream<Rm: PipeModeTag, Sm: PipeModeTag> {
    raw: RawPipeStream,
    _phantom: PhantomData<(Rm, Sm)>,
}

enum RawPipeStream {
    Server(TokioNPServer),
    Client(TokioNPClient),
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
    /// An error was reported by Tokio.
    ///
    /// Most of the time, this means that `from_raw_handle()` call was performed outside of the Tokio runtime, but OS errors associated with the registration of the handle in the runtime belong to this category as well.
    TokioError,
}
/// Error type for `from_raw_handle()` constructors.
pub type FromRawHandleError = (FromRawHandleErrorKind, io::Error);
