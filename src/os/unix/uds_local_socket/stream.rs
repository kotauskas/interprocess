use {
    crate::{
        error::ReuniteError,
        local_socket::{
            traits::{self, ReuniteResult},
            ConcurrencyDetector, LocalSocketSite, Name,
        },
        os::unix::{c_wrappers, uds_local_socket::dispatch_name},
        Sealed, TryClone,
    },
    std::{
        io::{self, prelude::*, IoSlice, IoSliceMut},
        os::{fd::OwnedFd, unix::net::UnixStream},
        sync::Arc,
    },
};

/// Wrapper around [`UnixStream`] that implements [`Stream`](traits::Stream).
#[derive(Debug)]
pub struct Stream(pub(super) UnixStream, ConcurrencyDetector<LocalSocketSite>);
impl Sealed for Stream {}
impl traits::Stream for Stream {
    type RecvHalf = RecvHalf;
    type SendHalf = SendHalf;

    fn connect(name: Name<'_>) -> io::Result<Self> {
        // TODO use nonblocking
        dispatch_name(name, false, |addr| c_wrappers::create_client(addr, false)).map(Self::from)
    }
    #[inline]
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    #[inline]
    fn split(self) -> (RecvHalf, SendHalf) {
        let arc = Arc::new(self);
        (RecvHalf(Arc::clone(&arc)), SendHalf(arc))
    }
    #[inline]
    #[allow(clippy::unwrap_in_result)]
    fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult<Self> {
        if !Arc::ptr_eq(&rh.0, &sh.0) {
            return Err(ReuniteError { rh, sh });
        }
        drop(rh);
        let inner = Arc::into_inner(sh.0).expect("stream half inexplicably copied");
        Ok(inner)
    }
}

impl Read for &Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let _guard = self.1.lock();
        (&mut &self.0).read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let _guard = self.1.lock();
        (&mut &self.0).read_vectored(bufs)
    }
    // FUTURE is_read_vectored
}
impl Write for &Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _guard = self.1.lock();
        (&mut &self.0).write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let _guard = self.1.lock();
        (&mut &self.0).write_vectored(bufs)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
    // FUTURE is_write_vectored
}

/// Access to the underlying implementation.
impl Stream {
    /// Borrows the [`UnixStream`] contained within, granting access to operations defined on it.
    #[inline(always)]
    pub fn inner(&self) -> &UnixStream { &self.0 }
    /// Mutably borrows the [`UnixStream`] contained within, granting access to operations defined
    /// on it.
    ///
    /// This may allow for non-portable concurrent I/O. Please use [`inner`](Self::inner) instead
    /// if you can.
    #[inline(always)]
    pub fn inner_mut(&mut self) -> &mut UnixStream { &mut self.0 }
}

/// Creates a fresh concurrency detector and thus may allow for non-portable concurrent I/O.
impl From<UnixStream> for Stream {
    fn from(s: UnixStream) -> Self { Self(s, ConcurrencyDetector::new()) }
}

impl From<OwnedFd> for Stream {
    fn from(fd: OwnedFd) -> Self { UnixStream::from(fd).into() }
}

impl TryClone for Stream {
    #[inline]
    fn try_clone(&self) -> std::io::Result<Self> { self.0.try_clone().map(Self::from) }
}

multimacro! {
    Stream,
    forward_asinto_handle(unix),
    derive_sync_mut_rw,
}

macro_rules! arc_accessors {
    ($ty:ty) => {
        /// [`Arc`] accessors.
        impl $ty {
            /// Borrows the [`Stream`] within the `Arc`.
            #[inline]
            pub fn as_stream(&self) -> &Stream { &self.0 }
            /// Extracts the underlying `Arc<Stream>`.
            #[inline]
            pub fn into_arc(self) -> Arc<Stream> { self.0 }
            /// Borrows the underlying `Arc<Stream>`, granting access to extra information about
            /// the `Arc`.
            #[inline]
            pub fn as_arc(&self) -> &Arc<Stream> { &self.0 }
        }
    };
}

/// [`Stream`]'s receive half, implemented using [`Arc`].
#[derive(Clone, Debug)]
pub struct RecvHalf(pub(super) Arc<Stream>);
impl Sealed for RecvHalf {}
impl traits::RecvHalf for RecvHalf {
    type Stream = Stream;
}
multimacro! {
    RecvHalf,
    forward_rbv(Stream, *),
    arc_accessors,
    forward_sync_ref_read,
    forward_as_handle,
    derive_sync_mut_read,
}

/// [`Stream`]'s send half, implemented using [`Arc`].
#[derive(Clone, Debug)]
pub struct SendHalf(pub(super) Arc<Stream>);
impl Sealed for SendHalf {}
impl traits::SendHalf for SendHalf {
    type Stream = Stream;
}
multimacro! {
    SendHalf,
    forward_rbv(Stream, *),
    arc_accessors,
    forward_sync_ref_write,
    forward_as_handle,
    derive_sync_mut_write,
}
