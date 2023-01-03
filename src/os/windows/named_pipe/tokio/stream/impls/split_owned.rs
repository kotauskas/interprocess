use super::*;

fn reunite<Rm: PipeModeTag, Sm: PipeModeTag>(
    rh: RecvHalf<Rm>,
    sh: SendHalf<Sm>,
) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
    if !Arc::ptr_eq(&rh.raw, &sh.raw) {
        return Err(ReuniteError {
            recv_half: rh,
            send_half: sh,
        });
    }
    drop(sh);
    let raw = Arc::try_unwrap(rh.raw).unwrap_or_else(|_| unreachable!("{}", UNWRAP_FAIL_MSG));
    Ok(PipeStream::new(raw))
}

impl<Rm: PipeModeTag> RecvHalf<Rm> {
    /// Attempts to reunite this receive half with the given send half to yield the original stream back, returning both halves as an error if they belong to different streams.
    #[inline]
    pub fn reunite<Sm: PipeModeTag>(self, other: SendHalf<Sm>) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
        reunite(self, other)
    }
    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeServerSessionId) }
    }
    /// Returns `true` if the underlying stream was created by a listener (server-side), `false` if it was created by connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        matches!(&*self.raw, RawPipeStream::Server(..))
    }
    /// Returns `true` if the underlying stream was created by connecting to a server (client-side), `false` if it was created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.is_server()
    }
}
impl RecvHalf<pipe_mode::Messages> {
    /// Receives a message from the pipe into the specified buffer, returning either the size of the message or a new buffer tailored to its size if it didn't fit into the buffer.
    ///
    /// See [`RecvResult`] for more on how the return value works. (Note that it's wrapped in `io::Result` – there's two levels of structures at play.)
    #[inline]
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf).await
    }
    /* // FIXME: currently impossible due to Tokio limitations.
    /// Same as [`.recv()`](Self::recv), but accepts an uninitialized buffer.
    #[inline]
    pub async fn recv_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf).await
    }
    */
    /// Attempts to receive a message from the pipe into the specified buffer. If it fits, it's written into the buffer, and if it doesn't, the buffer is unaffected. The return value indicates which of those two things happened and also contains the size of the message regardless of whether it was read or not.
    ///
    /// See [`TryRecvResult`] for a summary of how the return value works. (Note that it's wrapped in `io::Result` – there's two levels of structures at play.)
    #[inline]
    pub async fn try_recv(&self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        self.raw.try_recv_msg(buf).await
    }
    /* // FIXME: currently impossible due to Tokio limitations.
    /// Same as [`.try_recv()`](Self::try_recv), but accepts an uninitialized buffer.
    #[inline]
    pub async fn try_recv_to_uninit(
        &self,
        buf: &mut [MaybeUninit<u8>],
    ) -> io::Result<TryRecvResult> {
        self.raw.try_recv_msg(buf).await
    }
    */
}
impl AsyncRead for &RecvHalf<pipe_mode::Bytes> {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.raw.poll_read_init(cx, buf)
    }
}
impl AsyncRead for RecvHalf<pipe_mode::Bytes> {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.deref()).poll_read(cx, buf)
    }
}
impl<Rm: PipeModeTag> Debug for RecvHalf<Rm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("RecvHalf");
        self.raw.fill_fields(&mut dbst, Rm::MODE, None).finish()
    }
}
impl<Rm: PipeModeTag> AsRawHandle for RecvHalf<Rm> {
    #[inline(always)]
    fn as_raw_handle(&self) -> HANDLE {
        self.raw.as_raw_handle()
    }
}

impl<Sm: PipeModeTag> SendHalf<Sm> {
    fn ensure_flush_start(&self, slf_flush: &mut TokioMutexGuard<'_, Option<FlushJH>>) {
        if slf_flush.is_some() {
            return;
        }

        let handle = AssertHandleSyncSend(self.as_raw_handle());
        let task = tokio::task::spawn_blocking(move || {
            let handle = handle;
            FileHandle::flush_hndl(handle.0)
        });

        **slf_flush = Some(task);
    }
    /// Flushes the stream, waiting until the send buffer is empty (has been received by the other end in its entirety).
    pub async fn flush(&self) -> io::Result<()> {
        let mut slf_flush = self.flush.lock().await;
        let rslt = loop {
            match slf_flush.as_mut() {
                Some(fl) => break fl.await.unwrap(),
                None => self.ensure_flush_start(&mut slf_flush),
            }
        };
        *slf_flush = None;
        rslt
    }
    /// Attempts to reunite this send half with the given recieve half to yield the original stream back, returning both halves as an error if they belong to different streams.
    #[inline]
    pub fn reunite<Rm: PipeModeTag>(self, other: RecvHalf<Rm>) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
        reunite(other, self)
    }
    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.raw.as_raw_handle(), GetNamedPipeServerSessionId) }
    }
    /// Returns `true` if the underlying stream was created by a listener (server-side), `false` if it was created by connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        matches!(&*self.raw, RawPipeStream::Server(..))
    }
    /// Returns `true` if the underlying stream was created by connecting to a server (client-side), `false` if it was created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.is_server()
    }
}
impl SendHalf<pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically equal to the size of what was requested to be sent).
    #[inline]
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf).await
    }
}
impl AsyncWrite for &SendHalf<pipe_mode::Bytes> {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.raw.poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut lockfut = self.flush.lock();
        let lfpin = unsafe {
            // SAFETY: i promise,,,
            Pin::new_unchecked(&mut lockfut)
        };
        let mut slf_flush = ready!(lfpin.poll(cx));
        let rslt = loop {
            match slf_flush.as_mut() {
                Some(fl) => break ready!(Pin::new(fl).poll(cx)).unwrap(),
                None => self.ensure_flush_start(&mut slf_flush),
            }
        };
        *slf_flush = None;
        Poll::Ready(rslt)
    }
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for SendHalf<pipe_mode::Bytes> {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.deref()).poll_write(cx, buf)
    }
    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.deref()).poll_flush(cx)
    }
    #[inline]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.deref()).poll_close(cx)
    }
}
impl<Sm: PipeModeTag> Debug for SendHalf<Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("SendHalf");
        self.raw
            .fill_fields(&mut dbst, None, Sm::MODE)
            .field("flush", &self.flush)
            .finish()
    }
}
impl<Sm: PipeModeTag> AsRawHandle for SendHalf<Sm> {
    #[inline(always)]
    fn as_raw_handle(&self) -> HANDLE {
        self.raw.as_raw_handle()
    }
}
