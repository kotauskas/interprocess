//! Methods and trait implementations for `PipeStream`.
// TODO disconnect, as in PipeOps

mod split_owned;

use crate::os::windows::{
    imports::*,
    named_pipe::{
        convert_path, encode_to_utf16,
        new_stream::{
            block_for_server, has_msg_boundaries_from_sys, hget, is_server_from_sys, peek_msg_len, WaitTimeout,
            UNWRAP_FAIL_MSG,
        },
        tokio::new_stream::*,
        PipeMode,
    },
};
use std::{
    ffi::OsStr,
    fmt::{self, Debug, DebugStruct, Formatter},
    future::Future,
    mem::MaybeUninit,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

macro_rules! same_clsrv {
    ($nm:ident in $var:expr => $e:expr) => {
        match $var {
            RawPipeStream::Client($nm) => $e,
            RawPipeStream::Server($nm) => $e,
        }
    };
}

impl RawPipeStream {
    async fn wait_for_server(path: Vec<u16>) -> io::Result<Vec<u16>> {
        tokio::task::spawn_blocking(move || {
            block_for_server(&path, WaitTimeout::DEFAULT)?;
            Ok(path)
        })
        .await
        .expect("waiting for server panicked")
    }
    async fn connect(pipename: &OsStr, hostname: Option<&OsStr>, read: bool, write: bool) -> io::Result<Self> {
        let path = convert_path(pipename, hostname);
        let mut path16 = None::<Vec<u16>>;
        let client = loop {
            match _connect(&path, read, write) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let p16_take = match path16.take() {
                        Some(p) => p,
                        None => encode_to_utf16(&path),
                    };
                    let p16_take = Self::wait_for_server(p16_take).await?;
                    path16 = Some(p16_take);
                }
                not_waiting => break not_waiting?,
            }
        };
        Ok(Self::Client(client))
    }
    unsafe fn try_from_raw_handle(handle: HANDLE) -> Result<Self, FromRawHandleError> {
        let is_server = is_server_from_sys(handle).map_err(|e| (FromRawHandleErrorKind::IsServerCheckFailed, e))?;

        unsafe {
            match is_server {
                true => TokioNPServer::from_raw_handle(handle).map(Self::Server),
                false => TokioNPClient::from_raw_handle(handle).map(Self::Client),
            }
            .map_err(|e| (FromRawHandleErrorKind::TokioError, e))
        }
    }

    fn poll_read_readbuf(&mut self, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        same_clsrv!(x in self => Pin::new(x).poll_read(cx, buf))
    }

    // FIXME: silly Tokio doesn't support polling a named pipe through a shared reference, so this
    // has to be `&mut self`.
    fn poll_read_uninit(&mut self, cx: &mut Context<'_>, buf: &mut [MaybeUninit<u8>]) -> Poll<io::Result<usize>> {
        let mut readbuf = TokioReadBuf::uninit(buf);
        ready!(self.poll_read_readbuf(cx, &mut readbuf))?;
        Poll::Ready(Ok(readbuf.filled().len()))
    }
    #[inline]
    fn read_uninit<'a, 'b>(&'a mut self, buf: &'b mut [MaybeUninit<u8>]) -> ReadUninit<'a, 'b> {
        ReadUninit(self, buf)
    }

    fn poll_read_init(&self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        ready!(same_clsrv!(x in self => x.poll_read_ready(cx)))?;
        match same_clsrv!(x in self => x.try_read(buf)) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            els => Poll::Ready(els),
        }
    }
    #[inline]
    fn read_init<'a, 'b>(&'a self, buf: &'b mut [u8]) -> ReadInit<'a, 'b> {
        ReadInit(self, buf)
    }

    fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        ready!(same_clsrv!(x in self => x.poll_write_ready(cx)))?;
        match same_clsrv!(x in self => x.try_write(buf)) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            els => Poll::Ready(els),
        }
    }
    #[inline]
    fn write<'a>(&'a self, buf: &'a [u8]) -> Write<'a> {
        Write(self, buf)
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        same_clsrv!(x in self => Pin::new(x).poll_flush(cx))
    }
    fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        same_clsrv!(x in self => Pin::new(x).poll_shutdown(cx))
    }

    #[inline]
    fn try_recv_msg<'a, 'b>(&'a self, buf: &'b mut [u8]) -> TryRecvMsg<'a, 'b> {
        TryRecvMsg(self, buf)
    }
    async fn recv_msg(&self, buf: &mut [u8]) -> io::Result<RecvResult> {
        match self.try_recv_msg(buf).await?.to_result() {
            Err(sz) => {
                let mut buf = vec![0; sz];

                // FIXME: use read_uninit, see above.
                let len = self.read_init(&mut buf).await?;
                unsafe {
                    // SAFETY: Tokio's ReadBuf guarantees that at least this much is initialized.
                    buf.set_len(len)
                };
                Ok(RecvResult::Alloc(buf))
            }
            Ok(sz) => Ok(RecvResult::Fit(sz)),
        }
    }

    fn fill_fields<'a, 'b, 'c>(
        &self,
        dbst: &'a mut DebugStruct<'b, 'c>,
        readmode: Option<PipeMode>,
        writemode: Option<PipeMode>,
    ) -> &'a mut DebugStruct<'b, 'c> {
        let (tokio_object, is_server) = match self {
            RawPipeStream::Server(s) => (s as _, true),
            RawPipeStream::Client(c) => (c as _, false),
        };
        if let Some(readmode) = readmode {
            dbst.field("read_mode", &readmode);
        }
        if let Some(writemode) = writemode {
            dbst.field("write_mode", &writemode);
        }
        dbst.field("tokio_object", tokio_object).field("is_server", &is_server)
    }
}
impl AsRawHandle for RawPipeStream {
    fn as_raw_handle(&self) -> HANDLE {
        same_clsrv!(x in self => x.as_raw_handle())
    }
}

struct Write<'a>(&'a RawPipeStream, &'a [u8]);
impl Future for Write<'_> {
    type Output = io::Result<usize>;
    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        slf.0.poll_write(cx, slf.1)
    }
}

struct ReadUninit<'a, 'b>(&'a mut RawPipeStream, &'b mut [MaybeUninit<u8>]);
impl Future for ReadUninit<'_, '_> {
    type Output = io::Result<usize>;
    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        slf.0.poll_read_uninit(cx, slf.1)
    }
}

struct ReadInit<'a, 'b>(&'a RawPipeStream, &'b mut [u8]);
impl Future for ReadInit<'_, '_> {
    type Output = io::Result<usize>;
    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        slf.0.poll_read_init(cx, slf.1)
    }
}

struct TryRecvMsg<'a, 'b>(&'a RawPipeStream, &'b mut [u8]);
impl Future for TryRecvMsg<'_, '_> {
    type Output = io::Result<TryRecvResult>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        let (raw, buf) = (&mut slf.0, &mut slf.1);
        let size = peek_msg_len(raw.as_raw_handle())?;
        let fit = buf.len() >= size;
        if fit {
            ready!(Pin::new(raw).poll_read_init(cx, buf))?;
        }
        Poll::Ready(Ok(TryRecvResult { size, fit }))
    }
}

impl<Sm: PipeModeTag> PipeStream<pipe_mode::Messages, Sm> {
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
impl<Rm: PipeModeTag> PipeStream<Rm, pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically equal to the size of what was requested to be sent).
    #[inline]
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf).await
    }
}
impl<Sm: PipeModeTag> PipeStream<pipe_mode::Bytes, Sm> {
    /// Same as `.read()` from the [`Read`] trait, but accepts an uninitialized buffer.
    #[inline]
    pub async fn read_to_uninit(&mut self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.raw.read_uninit(buf).await
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe (the `\\.\pipe\` prefix is added automatically), blocking until a server instance is dispatched.
    pub async fn connect(pipename: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), None, Rm::MODE.is_some(), Sm::MODE.is_some()).await?;
        Ok(Self {
            raw,
            _phantom: PhantomData,
        })
    }
    /// Connects to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added automatically), blocking until a server instance is dispatched.
    pub async fn connect_to_remote(pipename: impl AsRef<OsStr>, hostname: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(
            pipename.as_ref(),
            Some(hostname.as_ref()),
            Rm::MODE.is_some(),
            Sm::MODE.is_some(),
        )
        .await?;
        Ok(Self {
            raw,
            _phantom: PhantomData,
        })
    }
    /// Splits the pipe stream by value, returning a receive half and a send half. The stream is closed when both are dropped, kind of like an `Arc` (I wonder how it's implemented under the hood...).
    pub fn split(self) -> (RecvHalf<Rm>, SendHalf<Sm>) {
        let raw_a = Arc::new(self.raw);
        let raw_ac = Arc::clone(&raw_a);
        (
            RecvHalf {
                raw: raw_a,
                _phantom: PhantomData,
            },
            SendHalf {
                raw: raw_ac,
                _phantom: PhantomData,
            },
        )
    }
    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_raw_handle(), GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_raw_handle(), GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_raw_handle(), GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_raw_handle(), GetNamedPipeServerSessionId) }
    }
    /// Returns `true` if the stream was created by a listener (server-side), `false` if it was created by connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        matches!(self.raw, RawPipeStream::Server(..))
    }
    /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if it was created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.is_server()
    }
    /// Attempts to wrap the given handle into the high-level pipe stream type. If the underlying pipe type is wrong or trying to figure out whether it's wrong or not caused a system call error, the corresponding error condition is returned.
    ///
    /// For more on why this can fail, see [`FromRawHandleError`]. Most notably, server-side write-only pipes will cause "access denied" errors because they lack permissions to check whether it's a server-side pipe and whether it has message boundaries.
    pub unsafe fn from_raw_handle(handle: HANDLE) -> Result<Self, FromRawHandleError> {
        let raw = unsafe {
            // SAFETY: safety contract is propagated.
            RawPipeStream::try_from_raw_handle(handle)?
        };
        // If the wrapper type tries to read incoming data as messages, that might break if
        // the underlying pipe has no message boundaries. Let's check for that.
        if Rm::MODE == Some(PipeMode::Messages) {
            let msg_bnd = has_msg_boundaries_from_sys(raw.as_raw_handle())
                .map_err(|e| (FromRawHandleErrorKind::MessageBoundariesCheckFailed, e))?;
            if !msg_bnd {
                return Err((
                    FromRawHandleErrorKind::NoMessageBoundaries,
                    io::Error::from(io::ErrorKind::InvalidInput),
                ));
            }
        }
        Ok(Self::new(raw))
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self {
            raw,
            _phantom: PhantomData,
        }
    }
}

impl<Sm: PipeModeTag> AsyncRead for &PipeStream<pipe_mode::Bytes, Sm> {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.raw.poll_read_init(cx, buf)
    }
}
impl<Sm: PipeModeTag> AsyncRead for PipeStream<pipe_mode::Bytes, Sm> {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.deref()).poll_read(cx, buf)
    }
}
impl<Sm: PipeModeTag> TokioAsyncRead for PipeStream<pipe_mode::Bytes, Sm> {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.get_mut().raw.poll_read_readbuf(cx, buf)
    }
}
impl<Rm: PipeModeTag> AsyncWrite for &PipeStream<Rm, pipe_mode::Bytes> {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.raw.poll_write(cx, buf)
    }
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
impl<Rm: PipeModeTag> AsyncWrite for PipeStream<Rm, pipe_mode::Bytes> {
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
impl<Rm: PipeModeTag> TokioAsyncWrite for PipeStream<Rm, pipe_mode::Bytes> {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        self.get_mut().raw.poll_write(cx, buf)
    }
    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.get_mut().raw.poll_flush(cx)
    }
    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.get_mut().raw.poll_shutdown(cx)
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Debug for PipeStream<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("PipeStream");
        self.raw.fill_fields(&mut dbst, Rm::MODE, Sm::MODE).finish()
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> AsRawHandle for PipeStream<Rm, Sm> {
    #[inline(always)]
    fn as_raw_handle(&self) -> HANDLE {
        self.raw.as_raw_handle()
    }
}
