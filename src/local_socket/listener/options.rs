#[cfg(feature = "tokio")]
use crate::local_socket::tokio::Listener as TokioListener;
#[cfg(windows)]
use crate::os::windows::security_descriptor::SecurityDescriptor;
use {
    crate::{
        local_socket::{traits, Listener, ListenerNonblockingMode, Name},
        Sealed, TryClone,
    },
    std::io,
};

/// A builder for [local socket listeners](traits::Listener), including [`Listener`].
#[derive(Debug)]
pub struct ListenerOptions<'n> {
    pub(crate) name: Name<'n>,
    pub(crate) nonblocking: ListenerNonblockingMode,
    pub(crate) reclaim_name: bool,
    #[cfg(unix)]
    pub(crate) mode: Option<libc::mode_t>,
    #[cfg(windows)]
    pub(crate) security_descriptor: Option<SecurityDescriptor>,
}
impl Sealed for ListenerOptions<'_> {}

impl TryClone for ListenerOptions<'_> {
    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            name: self.name.clone(),
            nonblocking: self.nonblocking,
            reclaim_name: self.reclaim_name,
            #[cfg(unix)]
            mode: self.mode,
            #[cfg(windows)]
            security_descriptor: self
                .security_descriptor
                .as_ref()
                .map(TryClone::try_clone)
                .transpose()?,
        })
    }
}

/// Creation.
impl ListenerOptions<'_> {
    /// Creates an options table with default values.
    #[inline]
    pub fn new() -> Self {
        Self {
            name: Name::invalid(),
            nonblocking: ListenerNonblockingMode::Neither,
            reclaim_name: true,
            #[cfg(unix)]
            mode: None,
            #[cfg(windows)]
            security_descriptor: None,
        }
    }
}

/// Option setters.
impl<'n> ListenerOptions<'n> {
    builder_setters! {
        /// Sets the name the server will listen on.
        name: Name<'n>,
        /// Selects the nonblocking mode to be used by the listener.
        ///
        /// The default value is `Neither`.
        nonblocking: ListenerNonblockingMode,
        /// Sets whether [name reclamation](Listener#name-reclamation) is to happen or not.
        ///
        /// This is enabled by default.
        reclaim_name: bool,
    }
}

/// Listener constructors.
impl ListenerOptions<'_> {
    /// Creates a [`Listener`], binding it to the specified local socket name.
    ///
    /// On platforms where there are multiple available implementations, this dispatches to the
    /// appropriate implementation based on where the name points to.
    #[inline]
    pub fn create_sync(self) -> io::Result<Listener> { self.create_sync_as::<Listener>() }
    /// Creates the given [type of listener](traits::Listener), binding it to the specified local
    /// socket name.
    #[inline]
    pub fn create_sync_as<L: traits::Listener>(self) -> io::Result<L> { L::from_options(self) }
    /// Creates a [`Listener`](TokioListener), binding it to the specified local socket name.
    ///
    /// On platforms where there are multiple available implementations, this dispatches to the
    /// appropriate implementation based on where the name points to.
    #[inline]
    #[cfg(feature = "tokio")]
    pub fn create_tokio(self) -> io::Result<TokioListener> {
        self.create_tokio_as::<TokioListener>()
    }
    /// Creates the given [type of listener](traits::tokio::Listener), binding it to the specified
    /// local socket name.
    #[inline]
    #[cfg(feature = "tokio")]
    pub fn create_tokio_as<L: traits::tokio::Listener>(self) -> io::Result<L> {
        L::from_options(self)
    }
}

impl Default for ListenerOptions<'_> {
    #[inline]
    fn default() -> Self { Self::new() }
}
