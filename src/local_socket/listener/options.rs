use crate::local_socket::{
	tokio::Listener as TokioListener, traits, Listener, ListenerNonblockingMode, Name,
};
use std::io;

/// A builder for [local socket listeners](traits::Listener), including [`Listener`].
#[derive(Clone, Debug)]
pub struct ListenerOptions<'n> {
	pub(crate) name: Name<'n>,
	pub(crate) nonblocking: ListenerNonblockingMode,
	pub(crate) reclaim_name: bool,
}

/// Options table creation and option setting.
impl<'n> ListenerOptions<'n> {
	/// Creates an options table with default values.
	#[inline]
	pub fn new() -> Self {
		Self {
			name: Name::default(),
			nonblocking: ListenerNonblockingMode::Neither,
			reclaim_name: true,
		}
	}

	// TODO docs, macro..?

	/// Sets the `name` option to the specified value.
	#[inline]
	pub fn name(mut self, name: Name<'n>) -> Self {
		self.name = name;
		self
	}
	/// Sets the `nonblocking` option to the specified value.
	#[inline]
	pub fn nonblocking(mut self, nonblocking: ListenerNonblockingMode) -> Self {
		self.nonblocking = nonblocking;
		self
	}
	/// Sets the `reclaim_name` option to the specified value.
	#[inline]
	pub fn reclaim_name(mut self, reclaim_name: bool) -> Self {
		self.reclaim_name = reclaim_name;
		self
	}
}

/// Listener constructors.
impl ListenerOptions<'_> {
	/// Creates a [`Listener`], binding it to the specified local socket name.
	///
	/// On platforms where there are multiple available implementations, this dispatches to the
	/// appropriate implementation based on where the name points to.
	#[inline]
	pub fn create_sync(self) -> io::Result<Listener> {
		self.create_sync_as::<Listener>()
	}
	/// Creates the given [type of listener](traits::Listener), binding it to the specified local
	/// socket name.
	#[inline]
	pub fn create_sync_as<L: traits::Listener>(self) -> io::Result<L> {
		L::from_options(self)
	}
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

impl<'n> Default for ListenerOptions<'n> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}
