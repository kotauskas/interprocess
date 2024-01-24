//! Methods and trait implementations for `PipeStream`.

use super::{
    limbo::{send_off, Corpse},
    *,
};
use crate::{
    os::windows::{
        decode_eof, downgrade_poll_eof,
        named_pipe::{
            path_conversion,
            stream::{block_for_server, has_msg_boundaries_from_sys, hget, is_server_from_sys, WaitTimeout},
            MaybeArc, NeedsFlushVal, PipeMode, PmtNotNone, DISCARD_BUF_SIZE, LIMBO_ERR, REBURY_ERR,
        },
        winprelude::*,
        FileHandle,
    },
    UnpinExt,
};
use recvmsg::{prelude::*, NoAddrBuf, RecvResult};
use std::{
    ffi::OsStr,
    fmt::{self, Debug, DebugStruct, Formatter},
    future::{self, Future},
    mem::{replace, ManuallyDrop, MaybeUninit},
    pin::Pin,
    sync::MutexGuard,
    task::{ready, Context, Poll},
};
use tokio::{
    io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf as TokioReadBuf},
    net::windows::named_pipe::{NamedPipeClient as TokioNPClient, NamedPipeServer as TokioNPServer},
};
use winapi::{
    shared::winerror::ERROR_MORE_DATA,
    um::winbase::{
        GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, GetNamedPipeServerProcessId,
        GetNamedPipeServerSessionId,
    },
};

macro_rules! same_clsrv {
    ($nm:ident in $var:expr => $e:expr) => {
        match $var {
            InnerTokio::Server($nm) => $e,
            InnerTokio::Client($nm) => $e,
        }
    };
}

#[repr(transparent)]
struct AssertHandleSyncSend(HANDLE);
unsafe impl Sync for AssertHandleSyncSend {}
unsafe impl Send for AssertHandleSyncSend {}

impl RawPipeStream {
    fn new(inner: InnerTokio) -> Self {
        Self {
            inner: Some(inner),
            needs_flush: NeedsFlush::from(NeedsFlushVal::No),
            recv_msg_state: Mutex::new(RecvMsgState::NotRecving),
        }
    }
    pub(crate) fn new_server(server: TokioNPServer) -> Self {
        Self::new(InnerTokio::Server(server))
    }
    fn new_client(client: TokioNPClient) -> Self {
        Self::new(InnerTokio::Client(client))
    }

    fn inner(&self) -> &InnerTokio {
        self.inner.as_ref().expect(LIMBO_ERR)
    }

    fn reap(&mut self) -> Corpse {
        self.inner.take().map(Corpse).expect(REBURY_ERR)
    }

    async fn wait_for_server(path: Vec<u16>) -> io::Result<Vec<u16>> {
        tokio::task::spawn_blocking(move || {
            block_for_server(&path, WaitTimeout::DEFAULT)?;
            Ok(path)
        })
        .await
        .expect("waiting for server panicked")
    }
    async fn connect(pipename: &OsStr, hostname: Option<&OsStr>, read: bool, write: bool) -> io::Result<Self> {
        let path = path_conversion::convert_path(pipename, hostname);
        let mut path16 = None::<Vec<u16>>;
        let client = loop {
            match _connect(&path, read, write) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let p16_take = match path16.take() {
                        Some(p) => p,
                        None => path_conversion::encode_to_utf16(&path),
                    };
                    let p16_take = Self::wait_for_server(p16_take).await?;
                    path16 = Some(p16_take);
                }
                not_waiting => break not_waiting?,
            }
        };
        Ok(Self::new_client(client))
    }

    fn poll_read_readbuf(&self, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        loop {
            match same_clsrv!(x in self.inner() => x.try_read_buf(buf)) {
                Ok(..) => return Poll::Ready(Ok(())),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            ready!(same_clsrv!(x in self.inner() => x.poll_read_ready(cx)))?;
        }
    }

    fn poll_read_uninit(&self, cx: &mut Context<'_>, buf: &mut [MaybeUninit<u8>]) -> Poll<io::Result<usize>> {
        let mut readbuf = TokioReadBuf::uninit(buf);
        ready!(downgrade_poll_eof(self.poll_read_readbuf(cx, &mut readbuf)))?;
        Poll::Ready(Ok(readbuf.filled().len()))
    }

    fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            ready!(same_clsrv!(x in self.inner() => x.poll_write_ready(cx)))?;
            match same_clsrv!(x in self.inner() => x.try_write(buf)) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                els => {
                    self.needs_flush.mark_dirty();
                    return Poll::Ready(els);
                }
            }
        }
    }

    fn poll_discard_msg(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut buf = [MaybeUninit::uninit(); DISCARD_BUF_SIZE];
        Poll::Ready(loop {
            match ready!(self.poll_read_uninit(cx, &mut buf)) {
                Ok(..) => break Ok(()),
                Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => {}
                Err(e) => break Err(e),
            }
        })
    }

    // TODO clarify in recvmsg that using different buffers across different polls of this function
    // that return Pending makes for unexpected behavior
    fn poll_recv_msg(
        &self,
        cx: &mut Context<'_>,
        buf: &mut MsgBuf<'_>,
        lock: Option<MutexGuard<'_, RecvMsgState>>,
    ) -> Poll<io::Result<RecvResult>> {
        // FIXME this stupid shit isn't reentrant; recursing on discard is gonna ruin everything
        let mut state = lock.unwrap_or_else(|| self.recv_msg_state.lock().unwrap());

        match &mut *state {
            RecvMsgState::NotRecving => {
                buf.set_fill(0);
                buf.has_msg = false;
                *state = RecvMsgState::Looping { spilled: false };
                self.poll_recv_msg(cx, buf, Some(state))
            }
            RecvMsgState::Looping { spilled } => {
                let mut more_data = true;
                while more_data {
                    let slice = buf.unfilled_part();
                    if slice.is_empty() {
                        match buf.grow() {
                            Ok(()) => {
                                *spilled = true;
                                debug_assert!(!buf.unfilled_part().is_empty());
                                continue;
                            }
                            Err(e) => {
                                if more_data {
                                    // A partially successful partial read must result in the rest of the
                                    // message being discarded.
                                    *state = RecvMsgState::Discarding {
                                        result: Ok(RecvResult::QuotaExceeded(e)),
                                    };
                                    return self.poll_recv_msg(cx, buf, Some(state));
                                }
                            }
                        }
                        continue;
                    }

                    let rslt = ready!(self.poll_read_uninit(cx, slice));

                    more_data = false;
                    let incr = match decode_eof(rslt) {
                        // FIXME Mio does the broken pipe thunking (this is a bug that breaks
                        // zero-sized messages)
                        Ok(0) => {
                            buf.set_fill(0);
                            return Poll::Ready(Ok(RecvResult::EndOfStream));
                        }
                        Ok(incr) => incr,
                        Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => {
                            more_data = true;
                            slice.len()
                        }
                        Err(e) => {
                            return if more_data {
                                // This is irrelevant to normal operation of downstream
                                // programs, but still makes them easier to debug.
                                *state = RecvMsgState::Discarding { result: Err(e) };
                                self.poll_recv_msg(cx, buf, Some(state))
                            } else {
                                Poll::Ready(Err(e))
                            };
                        }
                    };
                    unsafe {
                        // SAFETY: this one is on Tokio
                        buf.advance_init_and_set_fill(buf.len_filled() + incr)
                    };
                }

                let ret = if *spilled { RecvResult::Spilled } else { RecvResult::Fit };
                *state = RecvMsgState::NotRecving;
                Poll::Ready(Ok(ret))
            }
            RecvMsgState::Discarding { result } => {
                let _ = ready!(self.poll_discard_msg(cx));
                let r = replace(result, Ok(RecvResult::EndOfStream)); // Silly little sentinel...
                *state = RecvMsgState::NotRecving; // ...gone, so very young.
                Poll::Ready(r)
            }
        }
    }

    fn fill_fields<'a, 'b, 'c>(
        &self,
        dbst: &'a mut DebugStruct<'b, 'c>,
        readmode: Option<PipeMode>,
        writemode: Option<PipeMode>,
    ) -> &'a mut DebugStruct<'b, 'c> {
        let (tokio_object, is_server) = match self.inner() {
            InnerTokio::Server(s) => (s as _, true),
            InnerTokio::Client(c) => (c as _, false),
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
impl Drop for RawPipeStream {
    fn drop(&mut self) {
        let corpse = self.reap();
        if self.needs_flush.get() {
            send_off(corpse);
        }
    }
}
impl AsHandle for InnerTokio {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        same_clsrv!(x in self => x.as_handle())
    }
}
impl AsHandle for RawPipeStream {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.inner().as_handle()
    }
}
impl TryFrom<OwnedHandle> for RawPipeStream {
    type Error = FromHandleError;

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        let is_server = match is_server_from_sys(handle.as_handle()) {
            Ok(b) => b,
            Err(e) => {
                return Err(FromHandleError {
                    details: FromHandleErrorKind::IsServerCheckFailed,
                    cause: Some(e),
                    source: Some(handle),
                })
            }
        };

        let rh = handle.as_raw_handle();
        let handle = ManuallyDrop::new(handle);

        let tkresult = unsafe {
            match is_server {
                true => TokioNPServer::from_raw_handle(rh).map(InnerTokio::Server),
                false => TokioNPClient::from_raw_handle(rh).map(InnerTokio::Client),
            }
        };
        match tkresult {
            Ok(s) => Ok(Self::new(s)),
            Err(e) => Err(FromHandleError {
                details: FromHandleErrorKind::TokioError,
                cause: Some(e),
                source: Some(ManuallyDrop::into_inner(handle)),
            }),
        }
    }
}
// Tokio does not implement TryInto<OwnedHandle>
derive_asraw!(RawPipeStream);

impl<Rm: PipeModeTag> PipeStream<Rm, pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically
    /// equal to the size of what was requested to be sent).
    #[inline]
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        struct Write<'a>(&'a RawPipeStream, &'a [u8]);
        impl Future for Write<'_> {
            type Output = io::Result<usize>;
            #[inline]
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let slf = self.get_mut();
                slf.0.poll_write(cx, slf.1)
            }
        }
        Write(&self.raw, buf).await
    }
}

impl<Sm: PipeModeTag> PipeStream<pipe_mode::Bytes, Sm> {
    /// Same as `.read()` from [`AsyncReadExt`](::futures::AsyncReadExt), but accepts an uninitialized
    /// buffer.
    #[inline]
    pub async fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        struct ReadUninit<'a, 'b>(&'a RawPipeStream, &'b mut [MaybeUninit<u8>]);
        impl Future for ReadUninit<'_, '_> {
            type Output = io::Result<usize>;
            #[inline]
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let slf = self.get_mut();
                downgrade_poll_eof(slf.0.poll_read_uninit(cx, slf.1))
            }
        }
        ReadUninit(&self.raw, buf).await
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe (the `\\.\pipe\` prefix is added automatically), waiting until a server
    /// instance is dispatched.
    pub async fn connect(pipename: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), None, Rm::MODE.is_some(), Sm::MODE.is_some()).await?;
        Ok(Self::new(raw))
    }
    /// Connects to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added
    /// automatically), blocking until a server instance is dispatched.
    pub async fn connect_to_remote(pipename: impl AsRef<OsStr>, hostname: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(
            pipename.as_ref(),
            Some(hostname.as_ref()),
            Rm::MODE.is_some(),
            Sm::MODE.is_some(),
        )
        .await?;
        Ok(Self::new(raw))
    }
    /// Splits the pipe stream by value, returning a receive half and a send half. The stream is closed when both are
    /// dropped, kind of like an `Arc` (which is how it's implemented under the hood).
    pub fn split(mut self) -> (RecvPipeStream<Rm>, SendPipeStream<Sm>) {
        let (raw_ac, raw_a) = (self.raw.refclone(), self.raw);
        (
            RecvPipeStream {
                raw: raw_a,
                flush: None.into(), // PERF the mutex is unnecessary for readers
                _phantom: PhantomData,
            },
            SendPipeStream {
                raw: raw_ac,
                flush: self.flush,
                _phantom: PhantomData,
            },
        )
    }
    /// Attempts to reunite a receive half with a send half to yield the original stream back,
    /// returning both halves as an error if they belong to different streams (or when using
    /// this method on streams that were never split to begin with).
    pub fn reunite(rh: RecvPipeStream<Rm>, sh: SendPipeStream<Sm>) -> ReuniteResult<Rm, Sm> {
        if !MaybeArc::ptr_eq(&rh.raw, &sh.raw) {
            return Err(ReuniteError { rh, sh });
        }
        let PipeStream { mut raw, flush, .. } = sh;
        drop(rh);
        raw.try_make_owned();
        Ok(PipeStream {
            raw,
            flush,
            _phantom: PhantomData,
        })
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
    /// Returns `true` if the stream was created by a listener (server-side), `false` if it was created by connecting to
    /// a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        matches!(self.raw.inner(), &InnerTokio::Server(..))
    }
    /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if it was created by a
    /// listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.is_server()
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the thing from the wrong
    /// kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self {
            raw: MaybeArc::Inline(raw),
            flush: Mutex::new(None),
            _phantom: PhantomData,
        }
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag + PmtNotNone> PipeStream<Rm, Sm> {
    fn ensure_flush_start(&self, slf_flush: &mut MutexGuard<'_, Option<FlushJH>>) {
        if slf_flush.is_some() {
            return;
        }

        let handle = AssertHandleSyncSend(self.as_raw_handle());
        let task = tokio::task::spawn_blocking(move || FileHandle::flush_hndl({ handle }.0));

        **slf_flush = Some(task);
    }
    /// Flushes the stream, waiting until the send buffer is empty (has been received by the other end in its entirety).
    ///
    /// Only available on streams that have a send mode.
    #[inline]
    pub async fn flush(&self) -> io::Result<()> {
        future::poll_fn(|cx| self.poll_flush(cx)).await
    }

    /// Polls the future of `.flush()`.
    pub fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if !self.raw.needs_flush.on_flush() {
            // No flush required.
            return Poll::Ready(Ok(()));
        }

        let mut flush = self.flush.lock().unwrap();
        let rslt = loop {
            match flush.as_mut() {
                Some(fl) => break ready!(Pin::new(fl).poll(cx)).unwrap(),
                None => self.ensure_flush_start(&mut flush),
            }
        };
        *flush = None;
        if rslt.is_err() {
            self.raw.needs_flush.mark_dirty();
        }
        Poll::Ready(rslt)
    }

    /// Marks the stream as unflushed, preventing elision of the next flush operation (which
    /// includes limbo).
    #[inline]
    pub fn mark_dirty(&self) {
        self.raw.needs_flush.mark_dirty();
    }
    /// Assumes that the other side has consumed everything that's been written so far. This will turn the next flush
    /// into a no-op, but will cause the send buffer to be cleared when the stream is closed, since it won't be sent to
    /// limbo.
    ///
    /// If there's already an outstanding `.flush()` operation, it won't be affected by this call.
    #[inline]
    pub fn assume_flushed(&self) {
        self.raw.needs_flush.on_flush();
    }
    /// Drops the stream without sending it to limbo. This is the same as calling `assume_flushed()` right before
    /// dropping it.
    ///
    /// If there's already an outstanding `.flush()` operation, it won't be affected by this call.
    #[inline]
    pub fn evade_limbo(self) {
        self.assume_flushed();
    }
}

impl<Sm: PipeModeTag> TokioAsyncRead for &PipeStream<pipe_mode::Bytes, Sm> {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.get_mut().raw.poll_read_readbuf(cx, buf)
    }
}
impl<Sm: PipeModeTag> TokioAsyncRead for PipeStream<pipe_mode::Bytes, Sm> {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        TokioAsyncRead::poll_read(Pin::new(&mut &*self), cx, buf)
    }
}

impl<Rm: PipeModeTag> TokioAsyncWrite for &PipeStream<Rm, pipe_mode::Bytes> {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        self.get_mut().raw.poll_write(cx, buf)
    }
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.get_mut().poll_flush(cx)
    }
    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        // TODO actually close connection here
        TokioAsyncWrite::poll_flush(self, cx)
    }
}
impl<Rm: PipeModeTag> TokioAsyncWrite for PipeStream<Rm, pipe_mode::Bytes> {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        TokioAsyncWrite::poll_write((&mut &*self).pin(), cx, buf)
    }
    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        TokioAsyncWrite::poll_flush((&mut &*self).pin(), cx)
    }
    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        TokioAsyncWrite::poll_shutdown((&mut &*self).pin(), cx)
    }
}

impl<Sm: PipeModeTag> AsyncRecvMsg for &PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn poll_recv_msg(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut MsgBuf<'_>,
        _: Option<&mut NoAddrBuf>,
    ) -> Poll<io::Result<RecvResult>> {
        self.raw.poll_recv_msg(cx, buf, None)
    }
}
impl<Sm: PipeModeTag> AsyncRecvMsg for PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn poll_recv_msg(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut MsgBuf<'_>,
        _: Option<&mut NoAddrBuf>,
    ) -> Poll<io::Result<RecvResult>> {
        AsyncRecvMsg::poll_recv_msg((&mut &*self).pin(), cx, buf, None)
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> Debug for PipeStream<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("PipeStream");
        self.raw.fill_fields(&mut dbst, Rm::MODE, Sm::MODE);
        if Sm::MODE.is_some() {
            dbst.field("flush", &self.flush);
        }
        dbst.finish()
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> AsHandle for PipeStream<Rm, Sm> {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.raw.as_handle()
    }
}
/// Attempts to wrap the given handle into the high-level pipe stream type. If the underlying pipe type is wrong or
/// trying to figure out whether it's wrong or not caused a system call error, the corresponding error condition is
/// returned.
///
/// For more on why this can fail, see [`FromHandleError`]. Most notably, server-side write-only pipes will cause
/// "access denied" errors because they lack permissions to check whether it's a server-side pipe and whether it has
/// message boundaries.
impl<Rm: PipeModeTag, Sm: PipeModeTag> TryFrom<OwnedHandle> for PipeStream<Rm, Sm> {
    type Error = FromHandleError;

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        // If the wrapper type tries to read incoming data as messages, that might break if
        // the underlying pipe has no message boundaries. Let's check for that.
        if Rm::MODE == Some(PipeMode::Messages) {
            let msg_bnd = match has_msg_boundaries_from_sys(handle.as_handle()) {
                Ok(b) => b,
                Err(e) => {
                    return Err(FromHandleError {
                        details: FromHandleErrorKind::MessageBoundariesCheckFailed,
                        cause: Some(e),
                        source: Some(handle),
                    })
                }
            };
            if !msg_bnd {
                return Err(FromHandleError {
                    details: FromHandleErrorKind::NoMessageBoundaries,
                    cause: None,
                    source: Some(handle),
                });
            }
        }
        let raw = RawPipeStream::try_from(handle)?;
        Ok(Self::new(raw))
    }
}

derive_asraw!({Rm: PipeModeTag, Sm: PipeModeTag} PipeStream<Rm, Sm>, windows);
