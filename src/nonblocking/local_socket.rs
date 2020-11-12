//! Asynchronous local sockets.
//!
//! See the [blocking version of this module] for more on what those are.
//!
//! [blocking version of this module]: ../../local_socket/index.html " "

use blocking::{unblock, Unblock};
use futures::{
    stream::{unfold, Stream},
    AsyncRead, AsyncWrite,
};
use std::{
    io,
    sync::Arc,
};

use crate::local_socket::{self as sync, ToLocalSocketName};

/// An asynchronous local socket server, listening for connections.
///
/// # Example
/// ```no_run
/// use futures::{
///     io::{BufReader, AsyncBufReadExt, AsyncWriteExt},
///     stream::TryStreamExt,
/// };
/// use interprocess::nonblocking::local_socket::*;
///
/// let listener = LocalSocketListener::bind("/tmp/example.sock")
///     .await?;
/// listener
///     .incoming()
///     .try_for_each(|mut conn| async move {
///         conn.write_all(b"Hello from server!\n").await?;
///         let mut conn = BufReader::new(conn);
///         let mut buffer = String::new();
///         conn.read_line(&mut buffer).await?;
///         println!("Client answered: {}", buffer);
///         Ok(())
///     })
///     .await?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct LocalSocketListener {
    inner: Arc<sync::LocalSocketListener>,
}

impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    #[inline]
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
    #[inline]
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let s = self.inner.clone();
        Ok(LocalSocketStream {
            inner: Unblock::new(unblock(move || s.accept()).await?),
        })
    }
    /// Creates an infinite iterator which calls `accept()` with each iteration. Used together with `for` loops to conveniently create a main loop for a socket server.
    #[inline]
    pub fn incoming(&self) -> impl Stream<Item = std::io::Result<LocalSocketStream>> {
        // TODO (?) : effectively `clone`s the Arc twice for every Item.
        //      This could be fixed by copying the code from the `accept` function
        //      or creating an alernative `accept` function that takes `self`
        let s = self.inner.clone();
        unfold((), move |()| {
            let s = Arc::clone(&s);
            async move {
                Some((
                    unblock(move || s.accept())
                        .await
                        .map(|x| LocalSocketStream {
                            inner: Unblock::new(x),
                        }),
                    (),
                ))
            }
        })
    }
}

/// An asynchronous local socket byte stream, obtained eiter from [`LocalSocketListener`] or by connecting to an existing local socket.
///
/// # Example
/// ```no_run
/// use futures::io::{BufReader, AsyncBufReadExt, AsyncWriteExt};
/// use interprocess::nonblocking::local_socket::*;
///
/// // Replace the path as necessary on Windows.
/// let mut conn = LocalSocketStream::connect("/tmp/example.sock")
///     .await?;
/// conn.write_all(b"Hello from client!\n").await?;
/// let mut conn = BufReader::new(conn);
/// let mut buffer = String::new();
/// conn.read_line(&mut buffer).await?;
/// println!("Server answered: {}", buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
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

use futures::task::{Context, Poll};
use std::pin::Pin;
impl AsyncRead for LocalSocketStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        AsyncRead::poll_read(Pin::new(&mut self.inner), cx, buf)
    }
}
impl AsyncWrite for LocalSocketStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        AsyncWrite::poll_write(Pin::new(&mut self.inner), cx, buf)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), futures::io::Error>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.inner), cx)
    }
    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), futures::io::Error>> {
        AsyncWrite::poll_close(Pin::new(&mut self.inner), cx)
    }
}
