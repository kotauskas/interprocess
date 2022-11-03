//! Asynchronous local sockets.
//!
//! See the [blocking version of this module] for more on what those are.
//!
//! [blocking version of this module]: ../../local_socket/index.html " "

use super::imports::*;
use crate::local_socket::{self as sync, ToLocalSocketName};
use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// An asynchronous local socket server, listening for connections.
#[derive(Debug)]
pub struct LocalSocketListener {
    inner: Arc<sync::LocalSocketListener>,
}

impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    pub async fn bind<'a>(name: impl ToLocalSocketName<'_> + Send + 'static) -> io::Result<Self> {
        Ok(Self {
            inner: Arc::new(unblock(move || sync::LocalSocketListener::bind(name)).await?),
        })
    }
    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`incoming`] for a convenient way to create a main loop for a server.
    ///
    /// [`incoming`]: #method.incoming " "
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let s = self.inner.clone();
        Ok(LocalSocketStream {
            inner: Unblock::new(unblock(move || s.accept()).await?),
        })
    }
    /// Creates an infinite asynchronous stream which calls `accept()` with each iteration. Used together with [`for_each`]/[`try_for_each`] stream adaptors to conveniently create a main loop for a socket server.
    ///
    /// # Example
    /// See struct-level documentation for a complete example which already uses this method.
    ///
    /// [`for_each`]: https://docs.rs/futures/*/futures/stream/trait.StreamExt.html#method.for_each " "
    /// [`try_for_each`]: https://docs.rs/futures/*/futures/stream/trait.TryStreamExt.html#method.try_for_each " "
    pub fn incoming(&self) -> Incoming {
        Incoming {
            inner: Unblock::new(SyncArcIncoming {
                inner: Arc::clone(&self.inner),
            }),
        }
    }
}

/// An infinite asynchronous stream over incoming client connections of a [`LocalSocketListener`].
///
/// This stream is created by the [`incoming`] method on [`LocalSocketListener`] – see its documentation for more.
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
/// [`incoming`]: struct.LocalSocketListener.html#method.incoming " "
#[derive(Debug)]
pub struct Incoming {
    inner: Unblock<SyncArcIncoming>,
}
#[cfg(feature = "nonblocking")]
impl Stream for Incoming {
    type Item = Result<LocalSocketStream, io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let poll = <Unblock<_> as Stream>::poll_next(Pin::new(&mut self.inner), ctx);
        match poll {
            Poll::Ready(val) => {
                let val = val.map(|val| match val {
                    Ok(inner) => Ok(LocalSocketStream {
                        inner: Unblock::new(inner),
                    }),
                    Err(error) => Err(error),
                });
                Poll::Ready(val)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
#[cfg(feature = "nonblocking")]
impl FusedStream for Incoming {
    fn is_terminated(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct SyncArcIncoming {
    inner: Arc<sync::LocalSocketListener>,
}
impl Iterator for SyncArcIncoming {
    type Item = Result<sync::LocalSocketStream, io::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.accept())
    }
}

/// An asynchronous local socket byte stream, obtained eiter from [`LocalSocketListener`] or by connecting to an existing local socket.
#[derive(Debug)]
pub struct LocalSocketStream {
    inner: Unblock<sync::LocalSocketStream>,
}
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    pub async fn connect<'a>(
        name: impl ToLocalSocketName<'a> + Send + 'static,
    ) -> io::Result<Self> {
        Ok(Self {
            inner: Unblock::new(unblock(move || sync::LocalSocketStream::connect(name)).await?),
        })
    }
}

#[cfg(feature = "nonblocking")]
impl AsyncRead for LocalSocketStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        AsyncRead::poll_read(Pin::new(&mut self.inner), cx, buf)
    }
}
#[cfg(feature = "nonblocking")]
impl AsyncWrite for LocalSocketStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        AsyncWrite::poll_write(Pin::new(&mut self.inner), cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.inner), cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        AsyncWrite::poll_close(Pin::new(&mut self.inner), cx)
    }
}
