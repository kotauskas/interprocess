use crate::{
	local_socket::{tokio::stream::r#trait::Stream, Name},
	Sealed,
};
use futures_core::Stream as AsyncIterator;
use std::{
	future::Future,
	io,
	pin::Pin,
	task::{Context, Poll},
};

/// Tokio local socket server implementations.
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

	/// Asynchronously listens for incoming connections to the socket, returning a future that
	/// finishes only when a client is connected.
	///
	/// See [`.incoming()`](ListenerExt::incoming) for a convenient way to create a main loop for a
	/// server.
	fn accept(&self) -> impl Future<Output = io::Result<Self::Stream>> + Send + Sync;

	/// Disables [name reclamation](#name-reclamation) on the listener.
	// TODO link this
	fn do_not_reclaim_name_on_drop(&mut self);
}

/// Methods derived from the interface of [`Listener`].
pub trait ListenerExt: Listener {
	/// Creates an infinite [asynchronous iterator](AsyncIterator) which calls
	/// [`.accept()`](Listener::accept) with each iteration.
	///
	/// Used to conveniently create a main loop for a socket server.
	#[inline]
	fn incoming(&self) -> Incoming<'_, Self> {
		self.into()
	}
}
impl<T: Listener> ListenerExt for T {}

/// An infinite [asynchronous iterator](AsyncIterator) over incoming client connections of a
/// [`Listener`].
///
/// This str- *ahem,* **asynchronous iterator**, is created by the
/// [`incoming()`](ListenerExt::incoming) method on [`ListenerExt`] – see its documentation for
/// more.
#[derive(Debug)]
pub struct Incoming<'a, L> {
	listener: &'a L,
}
impl<'a, L: Listener> From<&'a L> for Incoming<'a, L> {
	fn from(listener: &'a L) -> Self {
		Self { listener }
	}
}

impl<L: Listener> AsyncIterator for Incoming<'_, L> {
	type Item = io::Result<L::Stream>;
	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let mut fut = self.get_mut().listener.accept();
		unsafe {
			// SAFETY: we aren't moving the future anywhere
			Pin::new_unchecked(&mut fut)
		}
		.poll(cx)
		.map(Some)
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(usize::MAX, None)
	}
}
