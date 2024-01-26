use super::*;
use crate::os::windows::named_pipe::{
    connect_without_waiting, path_conversion,
    stream::{block_for_server, WaitTimeout},
    MaybeArc, NeedsFlushVal, PipeMode,
};
use std::ffi::OsStr;

impl RawPipeStream {
    pub(super) fn new(inner: InnerTokio) -> Self {
        Self {
            inner: Some(inner),
            needs_flush: NeedsFlush::from(NeedsFlushVal::No),
            //recv_msg_state: Mutex::new(RecvMsgState::NotRecving),
        }
    }
    pub(crate) fn new_server(server: TokioNPServer) -> Self {
        Self::new(InnerTokio::Server(server))
    }
    fn new_client(client: TokioNPClient) -> Self {
        Self::new(InnerTokio::Client(client))
    }

    async fn wait_for_server(path: Vec<u16>) -> io::Result<Vec<u16>> {
        tokio::task::spawn_blocking(move || {
            block_for_server(&path, WaitTimeout::DEFAULT)?;
            Ok(path)
        })
        .await
        .expect("waiting for server panicked")
    }

    async fn connect(
        pipename: &OsStr,
        hostname: Option<&OsStr>,
        recv: Option<PipeMode>,
        send: Option<PipeMode>,
    ) -> io::Result<Self> {
        // FIXME should probably upstream FILE_WRITE_ATTRIBUTES for PipeMode::Messages to Tokio

        let mut path = Some(path_conversion::convert_and_encode_path(pipename, hostname));
        let client = loop {
            match connect_without_waiting(path.as_ref().unwrap(), recv, send, true) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let path_take = Self::wait_for_server(path.take().unwrap()).await?;
                    path = Some(path_take);
                }
                not_waiting => break not_waiting?,
            }
        };
        let client = unsafe { TokioNPClient::from_raw_handle(client.into_raw_handle())? };
        /* MESSAGE READING DISABLED
        if recv == Some(PipeMode::Messages) {
        set_named_pipe_handle_state(client.as_handle(), Some(PIPE_READMODE_MESSAGE), None, None)?;
        }
        */
        Ok(Self::new_client(client))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe (the `\\.\pipe\` prefix is added automatically),
    /// waiting until a server instance is dispatched.
    pub async fn connect(pipename: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), None, Rm::MODE, Sm::MODE).await?;
        Ok(Self::new(raw))
    }
    /// Connects to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix
    /// is added automatically), blocking until a server instance is dispatched.
    pub async fn connect_to_remote(
        pipename: impl AsRef<OsStr>,
        hostname: impl AsRef<OsStr>,
    ) -> io::Result<Self> {
        let raw =
            RawPipeStream::connect(pipename.as_ref(), Some(hostname.as_ref()), Rm::MODE, Sm::MODE)
                .await?;
        Ok(Self::new(raw))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the
    /// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self { raw: MaybeArc::Inline(raw), flush: Mutex::new(None), _phantom: PhantomData }
    }
}
