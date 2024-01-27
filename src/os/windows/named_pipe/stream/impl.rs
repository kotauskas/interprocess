//! Methods and trait implementations for `PipeStream`.

mod ctor;
mod debug;
mod handle;
mod recv_bytes;
mod recv_msg;
mod send;
mod send_off;

pub(crate) use send_off::{LIMBO_ERR, REBURY_ERR};

use super::*;
use crate::os::windows::{
    c_wrappers, decode_eof,
    named_pipe::{needs_flush::NeedsFlushVal, PipeMode},
    FileHandle,
};
use std::{
    io::{self, prelude::*},
    marker::PhantomData,
    mem::MaybeUninit,
};
use windows_sys::Win32::System::Pipes;

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
    /// Splits the pipe stream by value, returning a receive half and a send half. The stream is
    /// closed when both are dropped, kind of like an `Arc` (which is how it's implemented under the
    /// hood).
    pub fn split(mut self) -> (RecvPipeStream<Rm>, SendPipeStream<Sm>) {
        let (raw_ac, raw_a) = (self.raw.refclone(), self.raw);
        (
            RecvPipeStream { raw: raw_a, _phantom: PhantomData },
            SendPipeStream { raw: raw_ac, _phantom: PhantomData },
        )
    }
    /// Attempts to reunite a receive half with a send half to yield the original stream back,
    /// returning both halves as an error if they belong to different streams (or when using
    /// this method on streams that haven't been split to begin with).
    pub fn reunite(rh: RecvPipeStream<Rm>, sh: SendPipeStream<Sm>) -> ReuniteResult<Rm, Sm> {
        if !MaybeArc::ptr_eq(&rh.raw, &sh.raw) {
            return Err(ReuniteError { rh, sh });
        }
        let mut raw = sh.raw;
        drop(rh);
        raw.try_make_owned();
        Ok(PipeStream { raw, _phantom: PhantomData })
    }

    /// Retrieves the process identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), Pipes::GetNamedPipeClientProcessId) }
    }
    /// Retrieves the session identifier of the client side of the named pipe connection.
    #[inline]
    pub fn client_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), Pipes::GetNamedPipeClientSessionId) }
    }
    /// Retrieves the process identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_process_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), Pipes::GetNamedPipeServerProcessId) }
    }
    /// Retrieves the session identifier of the server side of the named pipe connection.
    #[inline]
    pub fn server_session_id(&self) -> io::Result<u32> {
        unsafe { hget(self.as_handle(), Pipes::GetNamedPipeServerSessionId) }
    }

    /// Returns `true` if the stream was created by a listener (server-side), `false` if it was
    /// created by connecting to a server (server-side).
    #[inline]
    pub fn is_server(&self) -> bool {
        self.raw.is_server
    }
    /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if
    /// it was created by a listener (server-side).
    #[inline]
    pub fn is_client(&self) -> bool {
        !self.raw.is_server
    }

    /// Sets whether the nonblocking mode for the pipe stream is enabled. By default, it is
    /// disabled.
    ///
    /// In nonblocking mode, attempts to receive from the pipe when there is no data available or to
    /// send when the buffer has filled up because the receiving side hasn't received enough bytes
    /// in time never block like they normally do. Instead, a
    /// [`WouldBlock`](io::ErrorKind::WouldBlock) error is immediately returned, allowing the thread
    /// to perform useful actions in the meantime.
    ///
    /// *If called on the server side, the flag will be set only for one stream instance.* A
    /// listener creation option, [`nonblocking`], and a similar method on the listener,
    /// [`.set_nonblocking()`], can be used to set the mode in bulk for all current instances and
    /// future ones.
    ///
    /// [`nonblocking`]: super::super::PipeListenerOptions::nonblocking
    /// [`.set_nonblocking()`]: super::super::PipeListener::set_nonblocking
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        super::set_nonblocking_given_readmode(self.as_handle(), nonblocking, Rm::MODE)
    }
}
