use crate::os::unix::udsocket::{
    ancwrap, c_wrappers,
    cmsg::{CmsgMut, CmsgMutBuf, CmsgRef},
    poll::{read_in_terms_of_vectored, write_in_terms_of_vectored},
    AsyncReadAncillary, AsyncWriteAncillary, ReadAncillarySuccess, ToUdSocketPath, UdSocket, UdSocketPath,
    UdStream as SyncUdStream,
};
use futures_core::ready;
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    error::Error,
    fmt::{self, Formatter},
    io,
    net::Shutdown,
    os::{fd::AsFd, unix::net::UnixStream as StdUdStream},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf as TokioReadBuf},
    net::{unix::ReuniteError as TokioReuniteError, UnixStream as TokioUdStream},
};

mod connect_future;
mod read_half;
mod write_half;
use connect_future::*;
pub use {read_half::*, write_half::*};

/// A Unix domain socket byte stream, obtained either from [`UdStreamListener`](super::UdStreamListener) or by connecting to an existing server.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::os::unix::udsocket::tokio::*;
/// use tokio::{
///     io::{AsyncReadExt, AsyncWriteExt},
///     try_join,
/// };
///
/// // Await this here since we can't do a whole lot without a connection.
/// let mut conn = UdStream::connect("/tmp/example.sock").await?;
///
/// // This takes an exclusive borrow of our connection and splits it into two
/// // halves, so that we could concurrently act on both. Take care not to use
/// // the .split() method from the futures crate's AsyncReadExt.
/// let (mut reader, mut writer) = conn.split();
///
/// // Allocate a sizeable buffer for reading.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the write operation as writing our whole string, waiting for
/// // that to complete, and then shutting down the write half, which sends
/// // an EOF to the other end to help it determine where the message ends.
/// let write = async {
///     writer.write_all(b"Hello from client!\n").await?;
///     writer.shutdown()?;
///     Ok(())
/// };
///
/// // Describe the read operation as reading until EOF into our big buffer.
/// let read = reader.read_to_string(&mut buffer);
///
/// // Concurrently perform both operations: write-and-send-EOF and read.
/// try_join!(write, read)?;
///
/// // Close the connection a bit earlier than you'd think we would. Nice practice!
/// drop(conn);
///
/// // Display the results when we're done!
/// println!("Server answered: {}", buffer.trim());
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct UdStream(TokioUdStream);
impl UdStream {
    /// Connects to a Unix domain socket server at the specified path.
    ///
    /// See [`ToUdSocketPath`] for an example of using various string types to specify socket paths.
    pub async fn connect(path: impl ToUdSocketPath<'_>) -> io::Result<Self> {
        let path = path.to_socket_path()?;
        Self::_connect(&path).await
    }
    async fn _connect(path: &UdSocketPath<'_>) -> io::Result<Self> {
        let stream = ConnectFuture { path }.await?;
        Self::try_from(stream).map_err(|e| e.cause.unwrap())
    }

    /// Splits a stream into a read half and a write half, which can be used to read and write the stream concurrently
    /// from independently spawned tasks, entailing a memory allocation.
    ///
    /// If borrowing is feasible, `UdStream` can simply be read from and written to by reference, no splitting required.
    ///
    /// Dropping either half will shut it down. This is equivalent to calling [`.shutdown()`](Self::shutdown) on the
    /// stream with the corresponding argument.
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (read_tok, write_tok) = self.0.into_split();
        (ReadHalf(read_tok), WriteHalf(write_tok))
    }
    /// Attempts to put two owned halves of a stream back together and recover the original stream. Succeeds only if the
    /// two halves originated from the same call to [`.split()`](Self::split).
    pub fn reunite(read: ReadHalf, write: WriteHalf) -> Result<Self, ReuniteError> {
        let (read_tok, write_tok) = (read.0, write.0);
        let stream_tok = read_tok.reunite(write_tok)?;
        Ok(Self::from(stream_tok))
    }

    fn pinproject(self: Pin<&mut Self>) -> Pin<&mut TokioUdStream> {
        Pin::new(&mut self.get_mut().0)
    }
}
tokio_wrapper_trait_impls!(
    for UdStream,
    sync SyncUdStream,
    std StdUdStream,
    tokio TokioUdStream);
derive_asraw!(unix: UdStream);

fn poll_read_ref(slf: &TokioUdStream, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
    loop {
        match slf.try_read_buf(buf) {
            Ok(..) => return Poll::Ready(Ok(())),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        ready!(slf.poll_read_ready(cx))?;
    }
}

fn poll_read_vec_ref(
    slf: &TokioUdStream,
    cx: &mut Context<'_>,
    bufs: &mut [io::IoSliceMut<'_>],
) -> Poll<io::Result<usize>> {
    // PERF should use readv instead
    poll_read_ancvec_ref(slf, cx, bufs, &mut CmsgMutBuf::new(&mut [])).map(|p| p.map(|s| s.main))
}

fn poll_read_ancvec_ref<AB: CmsgMut + ?Sized>(
    slf: &TokioUdStream,
    cx: &mut Context<'_>,
    bufs: &mut [io::IoSliceMut<'_>],
    abuf: &mut AB,
) -> Poll<io::Result<ReadAncillarySuccess>> {
    loop {
        match ancwrap::recvmsg(slf.as_fd(), bufs, abuf, None) {
            Ok(r) => return Poll::Ready(Ok(r)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        ready!(slf.poll_read_ready(cx))?;
    }
}

fn poll_write_ref(slf: &TokioUdStream, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    loop {
        match slf.try_write(buf) {
            Ok(s) => return Poll::Ready(Ok(s)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        ready!(slf.poll_write_ready(cx))?;
    }
}

fn poll_write_vec_ref(slf: &TokioUdStream, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
    loop {
        match slf.try_write_vectored(bufs) {
            Ok(s) => return Poll::Ready(Ok(s)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        ready!(slf.poll_write_ready(cx))?;
    }
}

fn poll_write_ancvec_ref(
    slf: &TokioUdStream,
    cx: &mut Context<'_>,
    bufs: &[io::IoSlice<'_>],
    abuf: CmsgRef<'_, '_>,
) -> Poll<io::Result<usize>> {
    loop {
        match ancwrap::sendmsg(slf.as_fd(), bufs, abuf) {
            Ok(r) => return Poll::Ready(Ok(r)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        ready!(slf.poll_write_ready(cx))?;
    }
}

impl TokioAsyncRead for &UdStream {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        poll_read_ref(&self.0, cx, buf)
    }
}

impl AsyncRead for &UdStream {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        <Self as TokioAsyncRead>::poll_read(self, cx, &mut buf).map(|p| p.map(|()| buf.filled().len()))
    }
    #[inline(always)]
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        poll_read_vec_ref(&self.0, cx, bufs)
    }
}

impl<AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for &UdStream {
    #[inline]
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        read_in_terms_of_vectored(self, cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        poll_read_ancvec_ref(&self.0, cx, bufs, abuf)
    }
}

impl TokioAsyncRead for UdStream {
    #[inline(always)]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut TokioReadBuf<'_>) -> Poll<io::Result<()>> {
        self.pinproject().poll_read(cx, buf)
    }
}

impl AsyncRead for UdStream {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut buf = TokioReadBuf::new(buf);
        self.pinproject()
            .poll_read(cx, &mut buf)
            .map(|p| p.map(|()| buf.filled().len()))
    }
    #[inline(always)]
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        <&Self as AsyncRead>::poll_read_vectored(Pin::new(&mut &*self), cx, bufs)
    }
}

impl<AB: CmsgMut + ?Sized> AsyncReadAncillary<AB> for UdStream {
    #[inline(always)]
    fn poll_read_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut &*self).poll_read_ancillary(cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_read_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        abuf: &mut AB,
    ) -> Poll<io::Result<ReadAncillarySuccess>> {
        Pin::new(&mut &*self).poll_read_ancillary_vectored(cx, bufs, abuf)
    }
}

impl TokioAsyncWrite for &UdStream {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        poll_write_ref(&self.0, cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.shutdown(Shutdown::Both)?;
        Poll::Ready(Ok(()))
    }

    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        poll_write_vec_ref(&self.0, cx, bufs)
    }
    /// True.
    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        true
    }
}

impl AsyncWrite for &UdStream {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        <Self as TokioAsyncWrite>::poll_write(self, cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdown(Shutdown::Both)?;
        Poll::Ready(Ok(()))
    }

    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        <Self as TokioAsyncWrite>::poll_write_vectored(self, cx, bufs)
    }
}

impl AsyncWriteAncillary for &UdStream {
    #[inline]
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        write_in_terms_of_vectored(self, cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        poll_write_ancvec_ref(&self.0, cx, bufs, abuf)
    }
}

impl TokioAsyncWrite for UdStream {
    #[inline(always)]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdown(Shutdown::Both)?;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for UdStream {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.pinproject().poll_write(cx, buf)
    }
    /// Does nothing and finishes immediately, as sockets cannot be flushed.
    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    /// Finishes immediately. See the `.shutdown()` method.
    #[inline(always)]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdown(Shutdown::Both)?;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWriteAncillary for UdStream {
    #[inline(always)]
    fn poll_write_ancillary(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write_ancillary(cx, buf, abuf)
    }
    #[inline(always)]
    fn poll_write_ancillary_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
        abuf: CmsgRef<'_, '_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write_ancillary_vectored(cx, bufs, abuf)
    }
}

/// Error indicating that a read half and a write half were not from the same stream, and thus could not be reunited.
#[derive(Debug)]
pub struct ReuniteError(pub ReadHalf, pub WriteHalf);
impl Error for ReuniteError {}
impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tried to reunite halves of different streams")
    }
}
impl From<TokioReuniteError> for ReuniteError {
    fn from(TokioReuniteError(read, write): TokioReuniteError) -> Self {
        let read = ReadHalf::from(read);
        let write = WriteHalf::from(write);
        Self(read, write)
    }
}
impl From<ReuniteError> for TokioReuniteError {
    fn from(ReuniteError(read, write): ReuniteError) -> Self {
        let read = read.into();
        let write = write.into();
        Self(read, write)
    }
}
