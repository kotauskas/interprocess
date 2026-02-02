#[cfg(feature = "tokio")]
use crate::local_socket::tokio::Stream as TokioStream;
#[cfg(feature = "tokio")]
use std::future::Future;
use {
    crate::{
        local_socket::{traits, Name, Stream},
        Sealed, TryClone,
    },
    std::{
        fmt::{self, Debug, Formatter},
        io,
    },
};

/// Client-side builder for [local socket streams](traits::Stream), including [`Stream`].
pub struct ConnectOptions<'n> {
    pub(crate) name: Name<'n>,
    flags: u8,
}
impl Sealed for ConnectOptions<'_> {}

const SHFT_NONBLOCKING_CONNECT: u8 = 0;
const SHFT_NONBLOCKING_STREAM: u8 = 1;
const ALL_BITS: u8 = (1 << 2) - 1;
#[allow(clippy::as_conversions)]
const fn set_bit(flags: u8, pos: u8, val: bool) -> u8 {
    flags & (ALL_BITS ^ (1 << pos)) | ((val as u8) << pos)
}
const fn has_bit(flags: u8, pos: u8) -> bool { flags & (1 << pos) != 0 }

impl TryClone for ConnectOptions<'_> {
    #[inline]
    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self { name: self.name.clone(), flags: self.flags })
    }
}

/// Creation and ownership.
impl ConnectOptions<'_> {
    /// Returns a default set of client options.
    #[inline]
    pub fn new() -> Self { Self { name: Name::invalid(), flags: 0 } }
}

/// Option setters.
impl<'n> ConnectOptions<'n> {
    builder_setters! {
        /// Sets the name the client will connect to.
        name: Name<'n>,
    }
    /// Sets whether the connection operation should be nonblocking or not.
    ///
    /// This is disabled by default.
    ///
    /// ## Platform-specific behavior
    /// ### Unix
    /// Number of additional `fcntl`s if `SOCK_NONBLOCK` is available:
    /// | conn \ stream | false | true |
    /// |---------------|-------|------|
    /// | false         |     0 |    1 |
    /// | true          |     1 |    0 |
    ///
    /// Number of additional `fcntl`s if `SOCK_NONBLOCK` is not available:
    /// | conn \ stream | false | true |
    /// |---------------|-------|------|
    /// | false         |     0 |    1 |
    /// | true          |     2 |    1 |
    ///
    /// ### Windows
    /// Has no effect, as attempting to connect to an overloaded named pipe will immediately
    /// return an error.
    #[must_use = builder_must_use!()]
    #[inline(always)]
    pub fn nonblocking_connect(mut self, nonblocking: bool) -> Self {
        self.flags = set_bit(self.flags, SHFT_NONBLOCKING_CONNECT, nonblocking);
        self
    }
    /// Sets whether the resulting connection is to have its reads and writes be nonblocking or
    /// not.
    ///
    /// This is disabled by default.
    ///
    /// ## Platform-specific behavior
    /// ### Unix
    /// See [`nonblocking_connect`](Self::nonblocking_connect).
    ///
    /// ### Windows
    /// The same as `.set_nonblocking(true)` immediately after creation.
    #[must_use = builder_must_use!()]
    #[inline(always)]
    pub fn nonblocking_stream(mut self, nonblocking: bool) -> Self {
        self.flags = set_bit(self.flags, SHFT_NONBLOCKING_STREAM, nonblocking);
        self
    }
}

/// Option getters.
impl ConnectOptions<'_> {
    pub(crate) fn get_nonblocking_connect(&self) -> bool {
        has_bit(self.flags, SHFT_NONBLOCKING_CONNECT)
    }
    pub(crate) fn get_nonblocking_stream(&self) -> bool {
        has_bit(self.flags, SHFT_NONBLOCKING_STREAM)
    }
}

/// Stream constructors.
impl ConnectOptions<'_> {
    /// Creates a [`Stream`] by connecting to the specified local socket name.
    ///
    /// On platforms where there are multiple available implementations, this dispatches to the
    /// appropriate implementation based on where the name points to.
    #[inline]
    pub fn connect_sync(&self) -> io::Result<Stream> { self.connect_sync_as::<Stream>() }
    /// Creates the given [type of stream](traits::Stream) by connecting to the specified local
    /// socket name.
    #[inline]
    pub fn connect_sync_as<S: traits::Stream>(&self) -> io::Result<S> { S::from_options(self) }
    /// Creates a Tokio [`Stream`](TokioStream) by connecting to the specified local socket name.
    ///
    /// On platforms where there are multiple available implementations, this dispatches to the
    /// appropriate implementation based on where the name points to.
    #[inline]
    #[cfg(feature = "tokio")]
    // FUTURE remove + '_
    pub fn connect_tokio(
        &self,
    ) -> impl Future<Output = io::Result<TokioStream>> + Send + Sync + '_ {
        self.connect_tokio_as::<TokioStream>()
    }
    /// Creates the given [type of Tokio stream](traits::tokio::Stream) by connecting to the
    /// specified local socket name.
    #[inline]
    #[cfg(feature = "tokio")]
    // FUTURE remove + '_
    pub fn connect_tokio_as<S: traits::tokio::Stream>(
        &self,
    ) -> impl Future<Output = io::Result<S>> + Send + Sync + '_ {
        S::from_options(self)
    }
}

impl Default for ConnectOptions<'_> {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl Debug for ConnectOptions<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectOptions")
            .field("name", &self.name)
            .field("nonblocking_connect", &self.get_nonblocking_connect())
            .field("nonblocking_stream", &self.get_nonblocking_stream())
            .finish()
    }
}
