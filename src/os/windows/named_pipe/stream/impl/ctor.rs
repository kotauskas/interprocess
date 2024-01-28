use std::{ffi::OsStr, path::Path};

use super::*;
use crate::os::windows::named_pipe::path_conversion::*;
use windows_sys::Win32::{Foundation::ERROR_PIPE_BUSY, System::Pipes::PIPE_READMODE_MESSAGE};

impl RawPipeStream {
    pub(super) fn new(handle: FileHandle, is_server: bool) -> Self {
        Self {
            handle: Some(handle),
            is_server,
            needs_flush: NeedsFlush::from(NeedsFlushVal::No),
            concurrency_detector: ConcurrencyDetector::new(),
        }
    }
    pub(crate) fn new_server(handle: FileHandle) -> Self {
        Self::new(handle, true)
    }
    fn new_client(handle: FileHandle) -> Self {
        Self::new(handle, false)
    }

    fn connect(path: &Path, recv: Option<PipeMode>, send: Option<PipeMode>) -> io::Result<Self> {
        Self::_connect(&encode_to_utf16(path.as_os_str()), recv, send)
    }

    fn connect_with_prepend(
        pipename: &OsStr,
        hostname: Option<&OsStr>,
        recv: Option<PipeMode>,
        send: Option<PipeMode>,
    ) -> io::Result<Self> {
        Self::_connect(&convert_and_encode_path(pipename, hostname), recv, send)
    }

    fn _connect(path: &[u16], recv: Option<PipeMode>, send: Option<PipeMode>) -> io::Result<Self> {
        let handle = loop {
            match connect_without_waiting(path, recv, send, false) {
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as _) => {
                    block_for_server(path, WaitTimeout::DEFAULT)?;
                    continue;
                }
                els => break els,
            }
        }?;

        if recv == Some(PipeMode::Messages) {
            set_named_pipe_handle_state(
                handle.as_handle(),
                Some(PIPE_READMODE_MESSAGE),
                None,
                None,
            )?;
        }
        Ok(Self::new_client(handle))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe at the specified path (the `\\<hostname>\pipe\` prefix
    /// is not added automatically), blocking until a server instance is dispatched.
    #[inline]
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        RawPipeStream::connect(path.as_ref(), Rm::MODE, Sm::MODE).map(Self::new)
    }

    #[inline]
    pub(crate) fn connect_with_prepend(
        pipename: &OsStr,
        hostname: Option<&OsStr>,
    ) -> io::Result<Self> {
        RawPipeStream::connect_with_prepend(pipename, hostname, Rm::MODE, Sm::MODE).map(Self::new)
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the
    /// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self { raw: raw.into(), _phantom: PhantomData }
    }
}
