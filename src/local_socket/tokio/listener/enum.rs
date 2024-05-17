use super::r#trait;
use crate::local_socket::{tokio::Stream, ListenerOptions};
#[cfg(unix)]
use crate::os::unix::uds_local_socket::tokio as uds_impl;
#[cfg(windows)]
use crate::os::windows::named_pipe::local_socket::tokio as np_impl;
use futures_core::{FusedStream as FusedAsyncIterator, Stream as AsyncIterator};
use std::{
	future::Future,
	io,
	pin::{pin, Pin},
	task::{Context, Poll},
};

impmod! {local_socket::dispatch_tokio as dispatch}

mkenum!(
/// Tokio-based local socket server, listening for connections.
///
/// This struct is created by [`ListenerOptions`](crate::local_socket::ListenerOptions).
///
/// [Name reclamation](super::super::Stream#name-reclamation) is performed by default on
/// backends that necessitate it.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
#[doc = doctest_file::include_doctest!("examples/local_socket/tokio/listener.rs")]
/// ```
Listener);

impl r#trait::Listener for Listener {
	type Stream = Stream;

	#[inline]
	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		dispatch::from_options(options)
	}
	#[inline]
	async fn accept(&self) -> io::Result<Stream> {
		dispatch!(Self: x in self => x.accept())
			.await
			.map(Stream::from)
	}
	#[inline]
	fn do_not_reclaim_name_on_drop(&mut self) {
		dispatch!(Self: x in self => x.do_not_reclaim_name_on_drop())
	}
}
impl AsyncIterator for Listener {
	type Item = io::Result<Stream>;
	#[inline(always)]
	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		pin!(r#trait::Listener::accept(self.get_mut()))
			.poll(cx)
			.map(Some)
	}
}
impl FusedAsyncIterator for Listener {
	#[inline(always)]
	fn is_terminated(&self) -> bool {
		false
	}
}
