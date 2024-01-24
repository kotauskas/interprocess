//! Methods and trait implementations for `PipeStream`.

// TODO split into a bunch of files

use super::{
    limbo::{send_off, Corpse},
    *,
};
use crate::{
    os::windows::{
        c_wrappers, decode_eof,
        named_pipe::{needs_flush::NeedsFlushVal, path_conversion, set_nonblocking_for_stream, PipeMode},
        FileHandle,
    },
    weaken_buf_init_mut, TryClone,
};
use recvmsg::{prelude::*, NoAddrBuf, RecvMsg, RecvResult};
use std::{
    ffi::OsStr,
    fmt::{self, Debug, DebugStruct, Formatter},
    io::{self, prelude::*},
    marker::PhantomData,
    mem::MaybeUninit,
    os::windows::prelude::*,
};
use winapi::{
    shared::winerror::ERROR_MORE_DATA,
    um::winbase::{
        GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, GetNamedPipeServerProcessId,
        GetNamedPipeServerSessionId,
    },
};

pub(crate) static LIMBO_ERR: &str = "attempt to perform operation on pipe stream which has been sent off to limbo";
pub(crate) static REBURY_ERR: &str = "attempt to bury same pipe stream twice";
pub(crate) const DISCARD_BUF_SIZE: usize = {
    // Debug builds are more prone to stack explosions.
    if cfg!(debug_assertions) {
        512
    } else {
        4096
    }
};

impl RawPipeStream {
    pub(crate) fn new(handle: FileHandle, is_server: bool) -> Self {
        Self {
            handle: Some(handle),
            is_server,
            needs_flush: NeedsFlush::from(NeedsFlushVal::No),
        }
    }
    pub(crate) fn new_server(handle: FileHandle) -> Self {
        Self::new(handle, true)
    }
    pub(crate) fn new_client(handle: FileHandle) -> Self {
        Self::new(handle, false)
    }

    fn file_handle(&self) -> &FileHandle {
        self.handle.as_ref().expect(LIMBO_ERR)
    }

    fn reap(&mut self) -> Corpse {
        Corpse {
            handle: self.handle.take().expect(REBURY_ERR),
            is_server: self.is_server,
        }
    }

    fn connect(pipename: &OsStr, hostname: Option<&OsStr>, read: bool, write: bool) -> io::Result<Self> {
        let path = path_conversion::convert_and_encode_path(pipename, hostname);
        let handle = _connect(&path, read, write, WaitTimeout::DEFAULT)?;
        Ok(Self::new_client(handle))
    }

    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_to_uninit(weaken_buf_init_mut(buf))
    }
    fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.file_handle().read(buf)
    }
    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let r = self.file_handle().write(buf);
        if r.is_ok() {
            self.needs_flush.mark_dirty();
        }
        r
    }

    fn flush(&self) -> io::Result<()> {
        if self.needs_flush.on_flush() {
            let r = self.file_handle().flush();
            if r.is_err() {
                self.needs_flush.mark_dirty();
            }
            r
        } else {
            Ok(())
        }
    }

    fn discard_msg(&self) -> io::Result<()> {
        // TODO not delegate to recv_msg
        use RecvResult::*;
        let mut bufbak = [MaybeUninit::uninit(); DISCARD_BUF_SIZE];
        let mut buf = MsgBuf::from(&mut bufbak[..]);
        buf.quota = Some(0);
        loop {
            match self.recv_msg_impl(&mut buf, false)? {
                EndOfStream | Fit => break,
                QuotaExceeded(..) => {
                    // Because discard = false makes sure that discard_msg() isn't recursed into,
                    // we have to manually reset the buffer into a workable state – by discarding
                    // the received data, that is.
                    buf.set_fill(0);
                }
                Spilled => unreachable!(),
            }
        }
        Ok(())
    }

    fn recv_msg_impl(&self, buf: &mut MsgBuf<'_>, discard: bool) -> io::Result<RecvResult> {
        buf.set_fill(0);
        buf.has_msg = false;
        let mut more_data = true;
        let mut spilled = false;
        let fh = self.file_handle();

        while more_data {
            let slice = buf.unfilled_part();
            if slice.is_empty() {
                match buf.grow() {
                    Ok(()) => {
                        spilled = true;
                        debug_assert!(!buf.unfilled_part().is_empty());
                        continue;
                    }
                    Err(e) => {
                        if more_data && discard {
                            // A partially successful partial read must result in the rest of the
                            // message being discarded.
                            let _ = self.discard_msg();
                        }
                        return Ok(RecvResult::QuotaExceeded(e));
                    }
                }
            }

            let rslt = fh.read(slice);

            more_data = false;
            let incr = match decode_eof(rslt) {
                Ok(incr) => incr,
                Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => {
                    more_data = true;
                    slice.len()
                }
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                    buf.set_fill(0);
                    return Ok(RecvResult::EndOfStream);
                }
                Err(e) => {
                    if more_data && discard {
                        // This is irrelevant to normal operation of downstream
                        // programs, but still makes them easier to debug.
                        let _ = self.discard_msg();
                    }
                    return Err(e);
                }
            };
            unsafe {
                // SAFETY: this one is on Windows
                buf.advance_init_and_set_fill(buf.len_filled() + incr)
            };
        }
        buf.has_msg = true;
        Ok(if spilled { RecvResult::Spilled } else { RecvResult::Fit })
    }

    #[inline]
    fn recv_msg(&self, buf: &mut MsgBuf<'_>) -> io::Result<RecvResult> {
        self.recv_msg_impl(buf, true)
    }

    fn set_nonblocking(&self, readmode: Option<PipeMode>, nonblocking: bool) -> io::Result<()> {
        unsafe { set_nonblocking_for_stream(self.as_handle(), readmode, nonblocking) }
    }

    fn fill_fields<'a, 'b, 'c>(
        &self,
        dbst: &'a mut DebugStruct<'b, 'c>,
        readmode: Option<PipeMode>,
        writemode: Option<PipeMode>,
    ) -> &'a mut DebugStruct<'b, 'c> {
        if let Some(readmode) = readmode {
            dbst.field("read_mode", &readmode);
        }
        if let Some(writemode) = writemode {
            dbst.field("write_mode", &writemode);
        }
        dbst.field("handle", &self.handle).field("is_server", &self.is_server)
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
impl AsHandle for RawPipeStream {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.file_handle().as_handle()
    }
}
derive_asraw!(RawPipeStream);
impl From<RawPipeStream> for OwnedHandle {
    #[inline]
    fn from(x: RawPipeStream) -> Self {
        x.into()
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
        Ok(Self::new(FileHandle(handle), is_server))
    }
}

impl<Rm: PipeModeTag> PipeStream<Rm, pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically equal to the size of
    /// what was requested to be sent).
    #[inline]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf)
    }
}
impl<Sm: PipeModeTag> PipeStream<pipe_mode::Bytes, Sm> {
    /// Same as `.read()` from the [`Read`] trait, but accepts an uninitialized buffer.
    #[inline]
    pub fn read_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.raw.read_to_uninit(buf)
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Connects to the specified named pipe (the `\\.\pipe\` prefix is added automatically), blocking until a server
    /// instance is dispatched.
    pub fn connect(pipename: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(pipename.as_ref(), None, Rm::MODE.is_some(), Sm::MODE.is_some())?;
        Ok(Self::new(raw))
    }
    /// Connects to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added
    /// automatically), blocking until a server instance is dispatched.
    pub fn connect_to_remote(pipename: impl AsRef<OsStr>, hostname: impl AsRef<OsStr>) -> io::Result<Self> {
        let raw = RawPipeStream::connect(
            pipename.as_ref(),
            Some(hostname.as_ref()),
            Rm::MODE.is_some(),
            Sm::MODE.is_some(),
        )?;
        Ok(Self::new(raw))
    }
    /// Splits the pipe stream by value, returning a receive half and a send half. The stream is closed when both are
    /// dropped, kind of like an `Arc` (which is how it's implemented under the hood).
    pub fn split(mut self) -> (RecvPipeStream<Rm>, SendPipeStream<Sm>) {
        let (raw_ac, raw_a) = (self.raw.refclone(), self.raw);
        (
            RecvPipeStream {
                raw: raw_a,
                _phantom: PhantomData,
            },
            SendPipeStream {
                raw: raw_ac,
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
        let mut raw = sh.raw;
        drop(rh);
        raw.try_make_owned();
        Ok(PipeStream {
            raw,
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
        self.raw.is_server
    }
    /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if it was created by a
    /// listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.raw.is_server
    }
    /// Sets whether the nonblocking mode for the pipe stream is enabled. By default, it is disabled.
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
    /// [`nonblocking`]: super::super::PipeListenerOptions::nonblocking
    /// [`set_nonblocking`]: super::super::PipeListener::set_nonblocking
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.raw.set_nonblocking(Rm::MODE, nonblocking)
    }

    /// Internal constructor used by the listener. It's a logic error, but not UB, to create the thing from the wrong
    /// kind of thing, but that never ever happens, to the best of my ability.
    pub(crate) fn new(raw: RawPipeStream) -> Self {
        Self {
            raw: raw.into(),
            _phantom: PhantomData,
        }
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag + PmtNotNone> PipeStream<Rm, Sm> {
    /// Flushes the stream, blocking until the send buffer is empty (has been received by the other
    /// end in its entirety).
    ///
    /// Only available on streams that have a send mode.
    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        self.raw.flush()
    }
    /// Marks the stream as unflushed, preventing elision of the next flush operation (which
    /// includes limbo).
    #[inline]
    pub fn mark_dirty(&self) {
        self.raw.needs_flush.mark_dirty();
    }
    /// Assumes that the other side has consumed everything that's been written so far. This will
    /// turn the next flush into a no-op, but will cause the send buffer to be cleared when the
    /// stream is closed, since it won't be sent to limbo.
    #[inline]
    pub fn assume_flushed(&self) {
        self.raw.needs_flush.on_flush();
    }
    /// Drops the stream without sending it to limbo. This is the same as calling
    /// `assume_flushed()` right before dropping it.
    #[inline]
    pub fn evade_limbo(self) {
        self.assume_flushed();
    }
}
impl<Sm: PipeModeTag> Read for &PipeStream<pipe_mode::Bytes, Sm> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.raw.read(buf)
    }
}
impl<Sm: PipeModeTag> Read for PipeStream<pipe_mode::Bytes, Sm> {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (self as &PipeStream<_, _>).read(buf)
    }
}
impl<Rm: PipeModeTag> Write for &PipeStream<Rm, pipe_mode::Bytes> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.raw.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.raw.flush()
    }
}
impl<Rm: PipeModeTag> Write for PipeStream<Rm, pipe_mode::Bytes> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self as &PipeStream<_, _>).write(buf)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        (self as &PipeStream<_, _>).flush()
    }
}

impl<Sm: PipeModeTag> RecvMsg for &PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(&mut self, buf: &mut MsgBuf<'_>, _: Option<&mut NoAddrBuf>) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf)
    }
}
impl<Sm: PipeModeTag> RecvMsg for PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(&mut self, buf: &mut MsgBuf<'_>, _: Option<&mut NoAddrBuf>) -> io::Result<RecvResult> {
        (&*self).recv_msg(buf, None)
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> Debug for PipeStream<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("PipeStream");
        self.raw.fill_fields(&mut dbst, Rm::MODE, Sm::MODE).finish()
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> AsHandle for PipeStream<Rm, Sm> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.raw.as_handle()
    }
}
/// Attempts to unwrap the given stream into the raw owned handle type, returning itself back if no
/// ownership over it is available, as is the case when the stream is split.
impl<Rm: PipeModeTag, Sm: PipeModeTag> TryFrom<PipeStream<Rm, Sm>> for OwnedHandle {
    type Error = PipeStream<Rm, Sm>;
    #[inline]
    fn try_from(s: PipeStream<Rm, Sm>) -> Result<Self, Self::Error> {
        match s.raw {
            MaybeArc::Inline(x) => Ok(x.into()),
            MaybeArc::Shared(..) => Err(s),
        }
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
        let raw = RawPipeStream::try_from(handle)?;
        // If the wrapper type tries to read incoming data as messages, that might break if
        // the underlying pipe has no message boundaries. Let's check for that.
        if Rm::MODE == Some(PipeMode::Messages) {
            let msg_bnd = match has_msg_boundaries_from_sys(raw.as_handle()) {
                Ok(b) => b,
                Err(e) => {
                    return Err(FromHandleError {
                        details: FromHandleErrorKind::MessageBoundariesCheckFailed,
                        cause: Some(e),
                        source: Some(raw.into()),
                    })
                }
            };
            if !msg_bnd {
                return Err(FromHandleError {
                    details: FromHandleErrorKind::NoMessageBoundaries,
                    cause: None,
                    source: Some(raw.into()),
                });
            }
        }
        Ok(Self::new(raw))
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> TryClone for PipeStream<Rm, Sm> {
    fn try_clone(&self) -> io::Result<Self> {
        let handle = c_wrappers::duplicate_handle(self.as_handle())?;
        self.raw.needs_flush.on_clone();
        Ok(Self::new(RawPipeStream {
            handle: Some(FileHandle(handle)),
            is_server: self.is_server(),
            needs_flush: NeedsFlush::from(NeedsFlushVal::Always),
        }))
    }
}

derive_asraw!({Rm: PipeModeTag, Sm: PipeModeTag} PipeStream<Rm, Sm>, windows);
