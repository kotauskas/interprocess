use crate::{
	local_socket::{stream::r#trait::Stream, Name},
	Sealed,
};
use std::{io, iter::FusedIterator};

/// Local socket server implementations.
///
/// Types on which this trait is implemented are variants of the
/// [`Listener` enum](super::enum::Listener). In addition, it is implemented on `Listener` itself,
/// which makes it a trait object of sorts. See its documentation for more on the semantics of the
/// methods seen here.
#[allow(private_bounds)]
pub trait Listener: Sized + Sealed {
	/// The stream type associated with this listener.
	type Stream: Stream;

	/// Creates a socket server with the specified local socket name.
	fn bind(name: Name<'_>) -> io::Result<Self>;

	/// Like [`bind()`](Listener::bind) followed by
	/// [`.do_not_reclaim_name_on_drop()`](Listener::do_not_reclaim_name_on_drop), but avoids a
	/// memory allocation.
	fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Self>;

	/// Listens for incoming connections to the socket, blocking until a client is connected.
	///
	/// See [`.incoming()`](ListenerExt::incoming) for a convenient way to create a main loop for a
	/// server.
	fn accept(&self) -> io::Result<Self::Stream>;

	/// Enables or disables the nonblocking mode for the listener. By default, it is disabled.
	///
	/// In the `Accept` and `Both` nonblocking modes, calling [`.accept()`] and iterating through
	/// [`.incoming()`] will immediately return a [`WouldBlock`](io::ErrorKind::WouldBlock) error
	/// if there is no client attempting to connect at the moment instead of blocking until one
	/// arrives.
	///
	/// In the `Stream` and `Both` nonblocking modes, the resulting stream will have nonblocking
	/// mode enabled.
	///
	/// [`.accept()`]: Listener::accept
	/// [`.incoming()`]: ListenerExt::incoming
	fn set_nonblocking(&self, nonblocking: ListenerNonblockingMode) -> io::Result<()>;

	/// Disables [name reclamation](super::enum::Listener#name-reclamation) on the listener.
	fn do_not_reclaim_name_on_drop(&mut self);
	// TODO ImplProperties type of thing
}

/// The manner in which a [listener](Listener) is to be nonblocking.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ListenerNonblockingMode {
	/// Neither `.accept()` nor the resulting stream are to have nonblocking semantics.
	Neither,
	/// `.accept()` will be nonblocking, but the resulting stream will not.
	Accept,
	/// The resulting stream will be nonblocking, but `.accept()` will not.
	Stream,
	/// Both `.accept()` and the resulting stream are to have nonblocking semantics.
	Both,
}
unsafe impl crate::ReprU8 for ListenerNonblockingMode {}

/// Methods derived from the interface of [`Listener`].
pub trait ListenerExt: Listener {
	/// Creates an infinite iterator which calls [`.accept()`](Listener::accept) with each
	/// iteration. Used together with `for` loops to conveniently create a main loop for a
	/// socket server.
	#[inline]
	fn incoming(&self) -> Incoming<'_, Self> {
		self.into()
	}
}
impl<T: Listener> ListenerExt for T {}

/// An infinite iterator over incoming client connections of a [`Listener`].
///
/// This iterator is created by the [`incoming()`](ListenerExt::incoming) method on
/// [`ListenerExt`] – see its documentation for more.
#[derive(Debug)]
pub struct Incoming<'a, L> {
	listener: &'a L,
}
impl<'a, L: Listener> From<&'a L> for Incoming<'a, L> {
	fn from(listener: &'a L) -> Self {
		Self { listener }
	}
}
impl<L: Listener> Iterator for Incoming<'_, L> {
	type Item = io::Result<L::Stream>;
	fn next(&mut self) -> Option<Self::Item> {
		Some(self.listener.accept())
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(usize::MAX, None)
	}
}
impl<L: Listener> FusedIterator for Incoming<'_, L> {}
