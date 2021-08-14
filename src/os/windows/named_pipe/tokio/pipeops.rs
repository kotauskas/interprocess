#![allow(clippy::unnecessary_mut_passed)] // We get &mut with mutexes either way

use super::imports::*;
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Mutex},
    task::{Context, Poll},
};
use to_method::To;

static LPE: &str = "unexpected lock poisoning";

macro_rules! l {
    ($e:expr) => {
        &mut *$e.lock().expect(LPE)
    };
}

pub enum PipeOps {
    // The mutexes can be replaced with UnsafeCell but I don't really feel like it.
    Client(Mutex<TokioNPClient>),
    Server(Mutex<TokioNPServer>),
}
impl PipeOps {
    /// Creates a `PipeOps` from a raw Windows API handle. The `server` argument specifies whether it should convert to a Tokio named pipe server struct or a client struct.
    ///
    /// # Safety
    /// See safety notes on Tokio's `from_raw_handle` on relevant types.
    pub unsafe fn from_raw_handle(handle: HANDLE, server: bool) -> io::Result<Self> {
        // SAFETY: as per safety contract
        let val = if server {
            Self::Server(unsafe { TokioNPServer::from_raw_handle(handle)? }.to::<Mutex<_>>())
        } else {
            Self::Client(unsafe { TokioNPClient::from_raw_handle(handle)? }.to::<Mutex<_>>())
        };
        Ok(val)
    }
    pub fn is_server(&self) -> bool {
        matches!(self, Self::Server(_))
    }
    pub fn is_client(&self) -> bool {
        matches!(self, Self::Client(_))
    }
    pub fn poll_read_readbuf(
        &self,
        ctx: &mut Context<'_>,
        buf: &mut TokioReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self {
            PipeOps::Client(c) => TokioAsyncRead::poll_read(Pin::new(l!(c)), ctx, buf),
            PipeOps::Server(s) => TokioAsyncRead::poll_read(Pin::new(l!(s)), ctx, buf),
        }
    }
    pub fn poll_read(&self, ctx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        futures::ready!(self.poll_read_readbuf(ctx, &mut buf))?;
        Poll::Ready(Ok(buf.filled().len()))
    }
    pub fn poll_write(&self, ctx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self {
            PipeOps::Client(c) => TokioAsyncWrite::poll_write(Pin::new(l!(c)), ctx, buf),
            PipeOps::Server(s) => TokioAsyncWrite::poll_write(Pin::new(l!(s)), ctx, buf),
        }
    }
    pub fn poll_flush(&self, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self {
            PipeOps::Client(c) => TokioAsyncWrite::poll_flush(Pin::new(l!(c)), ctx),
            PipeOps::Server(s) => TokioAsyncWrite::poll_flush(Pin::new(l!(s)), ctx),
        }
    }
    pub fn poll_shutdown(&self, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self {
            PipeOps::Client(c) => TokioAsyncWrite::poll_shutdown(Pin::new(l!(c)), ctx),
            PipeOps::Server(s) => TokioAsyncWrite::poll_shutdown(Pin::new(l!(s)), ctx),
        }
    }
    pub fn get_client_process_id(&self) -> io::Result<u32> {
        let mut id: u32 = 0;
        let success =
            unsafe { GetNamedPipeClientProcessId(self.as_raw_handle(), &mut id as *mut _) != 0 };
        if success {
            Ok(id)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn get_client_session_id(&self) -> io::Result<u32> {
        let mut id: u32 = 0;
        let success =
            unsafe { GetNamedPipeClientSessionId(self.as_raw_handle(), &mut id as *mut _) != 0 };
        if success {
            Ok(id)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn get_server_process_id(&self) -> io::Result<u32> {
        let mut id: u32 = 0;
        let success =
            unsafe { GetNamedPipeServerProcessId(self.as_raw_handle(), &mut id as *mut _) != 0 };
        if success {
            Ok(id)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn get_server_session_id(&self) -> io::Result<u32> {
        let mut id: u32 = 0;
        let success =
            unsafe { GetNamedPipeServerSessionId(self.as_raw_handle(), &mut id as *mut _) != 0 };
        if success {
            Ok(id)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub async fn connect_server(&self) -> io::Result<()> {
        match self {
            PipeOps::Client(_) => unimplemented!("connect_server() called on client PipeOps"),
            PipeOps::Server(s) => l!(s).connect().await,
        }
    }
    pub fn disconnect(&self) -> io::Result<()> {
        match self {
            PipeOps::Client(_) => {
                unimplemented!(
                    "named pipes on the client side cannot be disconnected without flushing"
                )
            }
            PipeOps::Server(s) => l!(s).disconnect(),
        }
    }
    pub fn server_drop_disconnect(&self) {
        let _ = self.disconnect();
    }
    // See the accept method body on the listener implementation for an explanation of what those
    // methods are for.
    pub async fn dry_read(&self) {
        DryRead(self).await;
    }
    pub async fn dry_write(&self) {
        DryWrite(self).await;
    }
}
#[cfg(windows)]
struct DryRead<'a>(&'a PipeOps);
#[cfg(windows)]
impl Future for DryRead<'_> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.poll_read(ctx, &mut []).map(|_| ())
    }
}
#[cfg(windows)]
struct DryWrite<'a>(&'a PipeOps);
#[cfg(windows)]
impl Future for DryWrite<'_> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.poll_write(ctx, &[]).map(|_| ())
    }
}

#[cfg(windows)]
impl AsRawHandle for PipeOps {
    fn as_raw_handle(&self) -> HANDLE {
        match self {
            PipeOps::Client(c) => l!(c).as_raw_handle(),
            PipeOps::Server(s) => l!(s).as_raw_handle(),
        }
    }
}

// Distinct from the non-async PipeStreamInternals which uses the non-async PipeOps.
pub trait PipeStreamInternals {
    fn build(instance: Arc<(PipeOps, AtomicBool)>) -> Self;
}
