use super::*;
use crate::os::windows::named_pipe::path_conversion;
use windows_sys::Win32::{Foundation::ERROR_PIPE_BUSY, System::Pipes::PIPE_READMODE_MESSAGE};

impl RawPipeStream {
    pub(super) fn new(handle: FileHandle, is_server: bool) -> Self {
        Self {
            handle: Some(handle),
            is_server,
            needs_flush: NeedsFlush::from(NeedsFlushVal::No),
        }
    }
    pub(crate) fn new_server(handle: FileHandle) -> Self {
        Self::new(handle, true)
    }
    fn new_client(handle: FileHandle) -> Self {
        Self::new(handle, false)
    }

    pub(super) fn connect(
        pipename: &OsStr,
        hostname: Option<&OsStr>,
        recv: Option<PipeMode>,
        send: Option<PipeMode>,
    ) -> io::Result<Self> {
        let path = path_conversion::convert_and_encode_path(pipename, hostname);
        let handle = loop {
            match connect_without_waiting(&path, recv, send, false) {
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as _) => {
                    block_for_server(&path, WaitTimeout::DEFAULT)?;
                    continue;
                }
                els => break els,
            }
        }?;

        if recv == Some(PipeMode::Messages) {
            set_named_pipe_handle_state(handle.as_handle(), Some(PIPE_READMODE_MESSAGE), None, None)?;
        }
        Ok(Self::new_client(handle))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe (the `\\.\pipe\` prefix is added automatically), blocking until a server
    /// instance is dispatched.
    pub fn connect(pipename: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), None, Rm::MODE, Sm::MODE)?;
        Ok(Self::new(raw))
    }
    /// Connects to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added
    /// automatically), blocking until a server instance is dispatched.
    pub fn connect_to_remote(pipename: impl AsRef<OsStr>, hostname: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), Some(hostname.as_ref()), Rm::MODE, Sm::MODE)?;
        Ok(Self::new(raw))
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the
    /// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self {
            raw: raw.into(),
            _phantom: PhantomData,
        }
    }
}
