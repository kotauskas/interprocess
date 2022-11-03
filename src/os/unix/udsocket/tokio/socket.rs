#[cfg(uds_peercred)]
use super::c_wrappers;
use {
    crate::os::unix::{imports::*, udsocket},
    std::{
        convert::TryFrom,
        future::Future,
        io,
        net::Shutdown,
        pin::Pin,
        task::{Context, Poll},
    },
    udsocket::{ToUdSocketPath, UdSocket as SyncUdSocket, UdSocketPath},
};

/// A Unix domain datagram socket, obtained either from [`UdSocketListener`](super::UdSocketListener) or by connecting to an existing server.
///
/// # Examples
/// - [Basic packet exchange](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_udsocket/inner.rs)
#[derive(Debug)]
pub struct UdSocket(TokioUdSocket);
impl UdSocket {
    /// Creates an unnamed datagram socket.
    pub fn unbound() -> io::Result<Self> {
        let socket = TokioUdSocket::unbound()?;
        Ok(Self(socket))
    }
    /// Creates a named datagram socket assigned to the specified path. This will be the "home" of this socket. Then, packets from somewhere else directed to this socket with [`.send_to()`] or [`.connect()`](Self::connect) will go here.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    pub fn bind<'a>(path: impl ToUdSocketPath<'a>) -> io::Result<Self> {
        Self::_bind(path.to_socket_path()?)
    }
    fn _bind(path: UdSocketPath<'_>) -> io::Result<Self> {
        let socket = TokioUdSocket::bind(path.as_osstr())?;
        Ok(Self(socket))
    }
    /// Selects the Unix domain socket to send packets to. You can also just use [`.send_to()`](Self::send_to) instead, but supplying the address to the kernel once is more efficient.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    pub fn set_destination<'a>(&self, path: impl ToUdSocketPath<'a>) -> io::Result<()> {
        self._set_destination(path.to_socket_path()?)
    }
    fn _set_destination(&self, path: UdSocketPath<'_>) -> io::Result<()> {
        self.0.connect(path.as_osstr())
    }
    /// Shuts down the read, write, or both halves of the socket. See [`Shutdown`].
    ///
    /// Attempting to call this method with the same `how` argument multiple times may return `Ok(())` every time or it may return an error the second time it is called, depending on the platform. You must either avoid using the same value twice or ignore the error entirely.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }
    /// Receives a single datagram from the socket, advancing the `ReadBuf` cursor by the datagram length.
    ///
    /// Uses Tokio's [`ReadBuf`] interface. See `.recv_stdbuf()` for a `&mut [u8]` version.
    pub async fn recv(&self, buf: &mut ReadBuf<'_>) -> io::Result<()> {
        // Tokio's .recv() uses &mut [u8] instead of &mut ReadBuf<'_> for some
        // reason, this works around that
        struct WrapperFuture<'a, 'b, 'c>(&'a UdSocket, &'b mut ReadBuf<'c>);
        impl Future for WrapperFuture<'_, '_, '_> {
            type Output = io::Result<()>;
            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.0 .0.poll_recv(cx, self.1)
            }
        }
        WrapperFuture(self, buf).await
    }
    /// Receives a single datagram from the socket, advancing the `ReadBuf` cursor by the datagram length.
    ///
    /// Uses an `std`-like `&mut [u8]` interface. See `.recv()` for a version which uses Tokio's [`ReadBuf`] instead.
    pub async fn recv_stdbuf(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf).await
    }
    /// Asynchronously waits until readable data arrives to the socket.
    ///
    /// May finish spuriously – *do not* perform a blocking read when this future finishes and *do* handle a [`WouldBlock`](io::ErrorKind::WouldBlock) or [`Poll::Pending`].
    pub async fn recv_ready(&self) -> io::Result<()> {
        self.0.readable().await
    }
    /// Sends a single datagram into the socket, returning how many bytes were actually sent.
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf).await
    }
    /// Sends a single datagram to the given address, returning how many bytes were actually sent.
    pub async fn send_to(&self, buf: &[u8], path: impl ToUdSocketPath<'_>) -> io::Result<usize> {
        let path = path.to_socket_path()?;
        self._send_to(buf, &path).await
    }
    async fn _send_to(&self, buf: &[u8], path: &UdSocketPath<'_>) -> io::Result<usize> {
        self.0.send_to(buf, path.as_osstr()).await
    }
    /// Asynchronously waits until the socket becomes writable due to the other side freeing up space in its OS receive buffer.
    ///
    /// May finish spuriously – *do not* perform a blocking write when this future finishes and *do* handle a [`WouldBlock`](io::ErrorKind::WouldBlock) or [`Poll::Pending`].
    pub async fn send_ready(&self) -> io::Result<()> {
        self.0.writable().await
    }
    /// Raw polling interface for receiving datagrams. You probably want `.recv()` instead.
    pub fn poll_recv(&self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        self.0.poll_recv(cx, buf)
    }
    /// Raw polling interface for receiving datagrams with an `std`-like receive buffer. You probably want `.recv_stdbuf()` instead.
    pub fn poll_recv_stdbuf(&self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<()>> {
        let mut readbuf = ReadBuf::new(buf);
        self.0.poll_recv(cx, &mut readbuf)
    }
    /// Raw polling interface for sending datagrams. You probably want `.send()` instead.
    pub fn poll_send(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.0.poll_send(cx, buf)
    }
    /// Raw polling interface for sending datagrams. You probably want `.send_to()` instead.
    pub fn poll_send_to<'a>(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        path: impl ToUdSocketPath<'a>,
    ) -> Poll<io::Result<usize>> {
        let path = path.to_socket_path()?;
        self._poll_send_to(cx, buf, &path)
    }
    fn _poll_send_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        path: &UdSocketPath<'_>,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_send_to(cx, buf, path.as_osstr())
    }
    /// Fetches the credentials of the other end of the connection without using ancillary data. The returned structure contains the process identifier, user identifier and group identifier of the peer.
    #[cfg(any(doc, uds_peercred))]
    #[cfg_attr( // uds_peercred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox",
            target_os = "haiku"
        )))
    )]
    pub fn get_peer_credentials(&self) -> io::Result<ucred> {
        c_wrappers::get_peer_ucred(self.as_raw_fd().as_ref())
    }
    tokio_wrapper_conversion_methods!(
        sync SyncUdSocket,
        std StdUdSocket,
        tokio TokioUdSocket);
}

tokio_wrapper_trait_impls!(
    for UdSocket,
    sync SyncUdSocket,
    std StdUdSocket,
    tokio TokioUdSocket);
