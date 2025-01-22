use {
    super::*,
    crate::os::windows::{named_pipe::WaitTimeout, path_conversion::*, NeedsFlushVal},
    std::{borrow::Cow, mem::take},
    widestring::U16CString,
};

impl RawPipeStream {
    pub(super) fn new(inner: InnerTokio, nfv: NeedsFlushVal) -> Self {
        Self {
            inner: Some(inner),
            needs_flush: NeedsFlush::from(nfv),
            //recv_msg_state: Mutex::new(RecvMsgState::NotRecving),
        }
    }
    pub(crate) fn new_server(server: TokioNPServer) -> Self {
        Self::new(InnerTokio::Server(server), NeedsFlushVal::No)
    }
    fn new_client(client: TokioNPClient) -> Self {
        Self::new(InnerTokio::Client(client), NeedsFlushVal::No)
    }

    async fn wait_for_server(path: U16CString) -> io::Result<U16CString> {
        tokio::task::spawn_blocking(move || {
            c_wrappers::block_for_server(&path, WaitTimeout::DEFAULT)?;
            Ok(path)
        })
        .await
        .expect("waiting for server panicked")
    }

    async fn connect(
        mut path: U16CString,
        recv: Option<PipeMode>,
        send: Option<PipeMode>,
    ) -> io::Result<Self> {
        let client = loop {
            match c_wrappers::connect_without_waiting(&path, recv, send, true) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let path_take = Self::wait_for_server(take(&mut path)).await?;
                    path = path_take;
                }
                not_waiting => break not_waiting?,
            }
        };
        let client = unsafe { TokioNPClient::from_raw_handle(client.into_raw_handle())? };
        /* MESSAGE READING DISABLED
        // FIXME(2.3.0) should probably upstream FILE_WRITE_ATTRIBUTES for PipeMode::Messages to Tokio
        if recv == Some(PipeMode::Messages) {
        set_named_pipe_handle_state(client.as_handle(), Some(PIPE_READMODE_MESSAGE), None, None)?;
        }
        */
        Ok(Self::new_client(client))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe at the specified path (the `\\<hostname>\pipe\` prefix
    /// is not added automatically), waiting until a server instance is dispatched.
    #[inline]
    pub async fn connect_by_path<'s>(path: impl ToWtf16<'s>) -> io::Result<Self> {
        RawPipeStream::connect(
            path.to_wtf_16().map(Cow::into_owned).map_err(to_io_error)?,
            Rm::MODE,
            Sm::MODE,
        )
        .await
        .map(Self::new)
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the
    /// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self {
            raw: MaybeArc::Inline(raw),
            flusher: Sm::TokioFlusher::default(),
            _phantom: PhantomData,
        }
    }
}
