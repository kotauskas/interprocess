//! Methods and trait implementations for `PipeStream`.

use super::{
    limbo::{send_off, Corpse},
    *,
};
use crate::{
    os::windows::{
        named_pipe::{path_conversion, set_nonblocking_for_stream, PipeMode},
        FileHandle,
    },
    reliable_recv_msg::{RecvResult, ReliableRecvMsg, TryRecvResult},
    weaken_buf_init_mut,
};
use std::{
    ffi::OsStr,
    fmt::{self, Debug, DebugStruct, Formatter},
    io::{self, prelude::*},
    marker::PhantomData,
    mem::MaybeUninit,
    os::windows::prelude::*,
    slice,
    sync::atomic::Ordering,
};
use windows_sys::Win32::{
    Foundation::ERROR_MORE_DATA,
    System::Pipes::{
        GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, GetNamedPipeServerProcessId,
        GetNamedPipeServerSessionId,
    },
};

/// Helper, used because `spare_capacity_mut()` on `Vec` is 1.60+. Borrows whole `Vec`, not just spare capacity.
#[inline]
pub(crate) fn vec_as_uninit(vec: &mut Vec<u8>) -> &mut [MaybeUninit<u8>] {
    let cap = vec.capacity();
    unsafe { slice::from_raw_parts_mut(vec.as_mut_ptr() as *mut MaybeUninit<u8>, cap) }
}

pub(crate) static LIMBO_ERR: &str = "attempt to perform operation on pipe stream which has been sent off to limbo";
pub(crate) static REBURY_ERR: &str = "attempt to bury same pipe stream twice";

impl RawPipeStream {
    pub(crate) fn new(handle: FileHandle, is_server: bool) -> Self {
        Self {
            handle: Some(handle),
            is_server,
            needs_flush: AtomicBool::new(false),
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
            self.needs_flush.store(true, Ordering::Release);
        }
        r
    }

    fn flush(&self) -> io::Result<()> {
        if self
            .needs_flush
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let r = self.file_handle().flush();
            if r.is_err() {
                self.needs_flush.store(true, Ordering::Release);
            }
            r
        } else {
            Ok(())
        }
    }

    fn assume_flushed(&self) {
        self.needs_flush.store(false, Ordering::Release);
    }

    fn try_recv_msg(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<TryRecvResult> {
        let mut size = 0;
        let mut fit = false;
        while size == 0 {
            size = peek_msg_len(self.as_handle())?;
            fit = buf.len() >= size;
            if fit {
                match self.file_handle().read(&mut buf[0..size]) {
                    // The ERROR_MORE_DATA here can only be hit if we're spinning in the loop and using the `.read()`
                    // to block until a message arrives, so that we could figure out for real if it fits or not.
                    // It doesn't mean that the message gets torn, as it normally does if the buffer given to the
                    // ReadFile call is non-zero in size.
                    Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => continue,
                    Err(e) => return Err(e),
                    Ok(nsz) => size = nsz,
                }
            } else {
                break;
            }
        }
        Ok(TryRecvResult { size, fit })
    }
    fn recv_msg(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<RecvResult> {
        let TryRecvResult { mut size, fit } = self.try_recv_msg(buf)?;
        if fit {
            Ok(RecvResult::Fit(size))
        } else {
            let mut buf = Vec::with_capacity(size);
            debug_assert!(buf.capacity() >= size);

            size = self.file_handle().read(vec_as_uninit(&mut buf))?;
            unsafe {
                // SAFETY: Win32 guarantees that at least this much is initialized.
                buf.set_len(size)
            };
            Ok(RecvResult::Alloc(buf))
        }
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
        if *self.needs_flush.get_mut() {
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

impl<Sm: PipeModeTag> PipeStream<pipe_mode::Messages, Sm> {
    /// Same as [`.recv()`](Self::recv), but accepts an uninitialized buffer.
    #[inline]
    pub fn recv_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf)
    }
    /// Same as [`.try_recv()`](Self::try_recv), but accepts an uninitialized buffer.
    #[inline]
    pub fn try_recv_to_uninit(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<TryRecvResult> {
        self.raw.try_recv_msg(buf)
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
    pub fn reunite(
        recver: RecvPipeStream<Rm>,
        sender: SendPipeStream<Sm>,
    ) -> Result<PipeStream<Rm, Sm>, ReuniteError<Rm, Sm>> {
        if !MaybeArc::ptr_eq(&recver.raw, &sender.raw) {
            return Err(ReuniteError {
                recv_half: recver,
                send_half: sender,
            });
        }
        let mut raw = sender.raw;
        drop(recver);
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
    /// [`set_nonblocking`]: super::super::PipeListenerOptions::set_nonblocking
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
impl<Sm: PipeModeTag> ReliableRecvMsg for &PipeStream<pipe_mode::Messages, Sm> {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult> {
        self.recv_to_uninit(weaken_buf_init_mut(buf))
    }
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        self.try_recv_to_uninit(weaken_buf_init_mut(buf))
    }
}
impl<Sm: PipeModeTag> ReliableRecvMsg for PipeStream<pipe_mode::Messages, Sm> {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult> {
        (self as &PipeStream<_, _>).recv(buf)
    }
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult> {
        (self as &PipeStream<_, _>).try_recv(buf)
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

derive_asraw!({Rm: PipeModeTag, Sm: PipeModeTag} PipeStream<Rm, Sm>, windows);
