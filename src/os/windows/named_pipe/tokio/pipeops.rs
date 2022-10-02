#![allow(clippy::unnecessary_mut_passed)] // We get &mut with mutexes either way

use {
    crate::os::windows::named_pipe::{
        tokio::{imports::*, stream::Instance},
        PipeOps as SyncPipeOps,
    },
    futures_core::ready,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, ErrorKind},
        task::{Context, Poll},
    },
};

macro_rules! same_clsrv {
    ($nm:ident in $var:ident : {$($t:tt)*}) => {
        match $var {
            PipeOps::Client($nm) => {$($t)*},
            PipeOps::Server($nm) => {$($t)*},
        }
    }
}

pub enum PipeOps {
    Client(TokioNPClient),
    Server(TokioNPServer),
}
impl PipeOps {
    /// Creates a `PipeOps` from a raw Windows API handle. The `server` argument specifies whether it should convert to a Tokio named pipe server struct or a client struct.
    ///
    /// # Safety
    /// See safety notes on Tokio's `from_raw_handle` on relevant types.
    pub unsafe fn from_raw_handle(handle: HANDLE, server: bool) -> io::Result<Self> {
        // SAFETY: as per safety contract
        let val = if server {
            Self::Server(unsafe { TokioNPServer::from_raw_handle(handle)? })
        } else {
            Self::Client(unsafe { TokioNPClient::from_raw_handle(handle)? })
        };
        Ok(val)
    }
    pub fn from_sync_pipeops(sync_pipeops: SyncPipeOps) -> io::Result<Self> {
        let is_server = sync_pipeops.is_server()?;
        let handle = sync_pipeops.into_raw_handle();
        let val = if is_server {
            Self::Server(unsafe { TokioNPServer::from_raw_handle(handle)? })
        } else {
            Self::Client(unsafe { TokioNPClient::from_raw_handle(handle)? })
        };
        Ok(val)
    }
    pub fn is_server(&self) -> bool {
        matches!(self, Self::Server(_))
    }
    pub fn is_client(&self) -> bool {
        matches!(self, Self::Client(_))
    }
    /* Gone because it requires a slightly newer version of Tokio than we can allow as per MSRV
    TODO: bump Tokio version and implement this
    pub fn poll_read_readbuf(
        &self,
        ctx: &mut Context<'_>,
        buf: &mut TokioReadBuf<'_>,
    ) -> Poll<io::Result<()>> {

    }
    */
    pub fn poll_read(&self, ctx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        same_clsrv!(s in self: {
            // Try to ready up right away to avoid spurious system calls.
            ready!(s.poll_read_ready(ctx))?;
            loop { // For as long as the read fails...
                // Check if the failure is because the data isn't ready yet...
                match s.try_read(buf) {
                    Err(e) if e.kind() == ErrorKind::WouldBlock => ready!(s.poll_read_ready(ctx))?,
                    // If it's not or we're not failing anymore, return verbatim.
                    els => return Poll::Ready(els),
                }
            }
        })
    }
    pub fn poll_write(&self, ctx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        same_clsrv!(s in self: {
            // Similar stuff here as above, try to ready up
            // right away to avoid spurious system calls.
            ready!(s.poll_write_ready(ctx))?;
            loop { // For as long as the read fails...
                // Check if the failure is because the buffer isn't empty enough yet...
                match s.try_write(buf) {
                    Err(e) if e.kind() == ErrorKind::WouldBlock => ready!(s.poll_read_ready(ctx))?,
                    // If it's not or we're not failing anymore, return verbatim.
                    els => return Poll::Ready(els),
                }
            }
        })
    }
    pub fn poll_flush(&self, _ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(())) // Nothing – Windows can't do async flush
    }
    pub fn poll_shutdown(&self, _ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(())) // nah
    }

    pub fn get_client_process_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeClientProcessId) }
    }
    pub fn get_client_session_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeClientSessionId) }
    }
    pub fn get_server_process_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeServerProcessId) }
    }
    pub fn get_server_session_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeServerSessionId) }
    }
    unsafe fn hget(
        &self,
        f: unsafe extern "system" fn(HANDLE, *mut u32) -> BOOL,
    ) -> io::Result<u32> {
        let mut x: u32 = 0;
        let success = unsafe { f(self.as_raw_handle(), &mut x as *mut _) != 0 };
        if success {
            Ok(x)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub async fn connect_server(&self) -> io::Result<()> {
        match self {
            PipeOps::Client(_) => unimplemented!("connect_server() called on client PipeOps"),
            PipeOps::Server(s) => s.connect().await,
        }
    }
    pub fn disconnect(&self) -> io::Result<()> {
        if let PipeOps::Server(s) = self {
            s.disconnect()?;
        }
        Ok(())
    }
    pub fn server_drop_disconnect(&self) {
        self.disconnect()
            .expect("failed to disconnect server from client");
    }
}
impl Debug for PipeOps {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        same_clsrv!(s in self: { Debug::fmt(s, f) })
    }
}

#[cfg(windows)]
impl AsRawHandle for PipeOps {
    fn as_raw_handle(&self) -> HANDLE {
        same_clsrv!(s in self: { s.as_raw_handle() })
    }
}

// Distinct from the non-async PipeStreamInternals which uses the non-async PipeOps.
pub trait PipeStreamInternals {
    #[cfg(windows)]
    fn build(instance: Instance) -> Self;
}
