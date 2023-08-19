use super::*;

pub(crate) static UNWRAP_FAIL_MSG: &str =
    "reference counter unwrap failed, even though the other half has just been dropped";

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
    Ok(PipeStream {
        raw,
        _phantom: PhantomData,
    })
}

impl<Rm: PipeModeTag> RecvHalf<Rm> {
    /// Attempts to reunite this receive half with the given send half to yield the original stream back, returning both
    /// halves as an error if they belong to different streams.
    #[inline]
    pub fn reunite<Sm: PipeModeTag>(self, other: SendHalf<Sm>) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
        reunite(self, other)
    }
    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeServerSessionId) }
    }
    /// Returns `true` if the underlying stream was created by a listener (server-side), `false` if it was created by
    /// connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        self.raw.is_server
    }
    /// Returns `true` if the underlying stream was created by connecting to a server (client-side), `false` if it was
    /// created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.raw.is_server
    }
    /// Sets whether the nonblocking mode for the whole pipe stream is enabled. **Note that this also affects the
    /// associated send half.** By default, it is disabled.
    ///
    /// In nonblocking mode, attempts to read from the pipe when there is no data available or to write when the buffer
    /// has filled up because the receiving side did not read enough bytes in time will never block like they normally
    /// do. Instead, a [`WouldBlock`](io::ErrorKind::WouldBlock) error is immediately returned, allowing the thread to
    /// perform useful actions in the meantime.
    ///
    /// *If called on the server side, the flag will be set only for one stream instance.* A listener creation option,
    /// [`nonblocking`], and a similar method on the listener, [`set_nonblocking`], can be used to set the mode in bulk
    /// for all current instances and future ones.
    ///
    /// [`nonblocking`]: crate::os::windows::named_pipe::PipeListenerOptions::nonblocking
    /// [`set_nonblocking`]: crate::os::windows::named_pipe::PipeListenerOptions::set_nonblocking
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.raw.set_nonblocking(Rm::MODE, nonblocking)
    }
}
impl RecvHalf<pipe_mode::Messages> {
    /// Same as [`.recv()`](ReliableRecvMsg::recv), but accepts an uninitialized buffer.
    #[inline]
    pub fn recv_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf)
    }
    /// Same as [`.try_recv()`](ReliableRecvMsg::try_recv), but accepts an uninitialized buffer.
    #[inline]
    pub fn try_recv_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<TryRecvResult> {
        self.raw.try_recv_msg(buf)
    }
}
impl RecvHalf<pipe_mode::Bytes> {
    /// Same as `.read()` from the [`Read`] trait, but accepts an uninitialized buffer.
    #[inline]
    pub fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.raw.read_to_uninit(buf)
    }
}
impl Read for &RecvHalf<pipe_mode::Bytes> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.raw.read(buf)
    }
}
impl Read for RecvHalf<pipe_mode::Bytes> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (self as &RecvHalf<_>).read(buf)
    }
}
impl ReliableRecvMsg for &RecvHalf<pipe_mode::Messages> {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult> {
        self.recv_to_uninit(weaken_buf_init_mut(buf))
    }
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        self.try_recv_to_uninit(weaken_buf_init_mut(buf))
    }
}
impl ReliableRecvMsg for RecvHalf<pipe_mode::Messages> {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult> {
        (self as &RecvHalf<_>).recv(buf)
    }
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        (self as &RecvHalf<_>).try_recv(buf)
    }
}
impl<Rm: PipeModeTag> Debug for RecvHalf<Rm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("RecvHalf");
        self.raw.fill_fields(&mut dbst, Rm::MODE, None).finish()
    }
}
impl<Rm: PipeModeTag> AsHandle for RecvHalf<Rm> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.raw.as_handle()
    }
}
derive_asraw!(windows: {Rm: PipeModeTag} RecvHalf<Rm>);

impl<Sm: PipeModeTag> SendHalf<Sm> {
    /// Flushes the stream, blocking until the send buffer is empty (has been received by the other end in its
    /// entirety).
    ///
    /// Only available on streams that have a send mode.
    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        self.raw.flush()
    }
    /// Assumes that the other side has consumed everything that's been written so far. This will turn the next flush
    /// into a no-op, but will cause the send buffer to be cleared when the stream is closed, since it won't be sent to
    /// limbo.
    #[inline]
    pub fn assume_flushed(&self) {
        self.raw.assume_flushed()
    }
    /// Drops the stream without sending it to limbo. This is the same as calling `assume_flushed()` right before
    /// dropping it.
    pub fn evade_limbo(self) {
        self.assume_flushed();
    }
    /// Attempts to reunite this send half with the given receive half to yield the original stream back, returning both
    /// halves as an error if they belong to different streams.
    #[inline]
    pub fn reunite<Rm: PipeModeTag>(self, other: RecvHalf<Rm>) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
        reunite(other, self)
    }
    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), GetNamedPipeServerSessionId) }
    }
    /// Returns `true` if the underlying stream was created by a listener (server-side), `false` if it was created by
    /// connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        self.raw.is_server
    }
    /// Returns `true` if the underlying stream was created by connecting to a server (client-side), `false` if it was
    /// created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.raw.is_server
    }
    /// Sets whether the nonblocking mode for the whole pipe stream is enabled. **Note that this also affects the
    /// associated receive half.** By default, it is disabled.
    ///
    /// In nonblocking mode, attempts to read from the pipe when there is no data available or to write when the buffer
    /// has filled up because the receiving side did not read enough bytes in time will never block like they normally
    /// do. Instead, a [`WouldBlock`](io::ErrorKind::WouldBlock) error is immediately returned, allowing the thread to
    /// perform useful actions in the meantime.
    ///
    /// *If called on the server side, the flag will be set only for one stream instance.* A listener creation option,
    /// [`nonblocking`], and a similar method on the listener, [`set_nonblocking`], can be used to set the mode in bulk
    /// for all current instances and future ones.
    ///
    /// [`nonblocking`]: crate::os::windows::named_pipe::PipeListenerOptions::nonblocking
    /// [`set_nonblocking`]: crate::os::windows::named_pipe::PipeListenerOptions::set_nonblocking
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.raw.set_nonblocking(Sm::MODE, nonblocking)
    }
}
impl SendHalf<pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically equal to the size of
    /// what was requested to be sent).
    #[inline]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf)
    }
}
impl Write for &SendHalf<pipe_mode::Bytes> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.raw.flush()
    }
}
impl Write for SendHalf<pipe_mode::Bytes> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self as &SendHalf<_>).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (self as &SendHalf<_>).flush()
    }
}
impl<Sm: PipeModeTag> Debug for SendHalf<Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("SendHalf");
        self.raw.fill_fields(&mut dbst, None, Sm::MODE).finish()
    }
}
impl<Sm: PipeModeTag> AsHandle for SendHalf<Sm> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.raw.as_handle()
    }
}
derive_asraw!(windows: {Sm: PipeModeTag} SendHalf<Sm>);
