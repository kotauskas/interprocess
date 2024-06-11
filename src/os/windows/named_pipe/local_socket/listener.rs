use super::stream::Stream;
use crate::{
	local_socket::{
		traits::{self, ListenerNonblockingMode, Stream as _},
		ListenerOptions, NameInner,
	},
	os::windows::named_pipe::{pipe_mode::Bytes, PipeListener, PipeListenerOptions},
	AtomicEnum, Sealed,
};
use std::{io, iter::FusedIterator, os::windows::prelude::*, sync::atomic::Ordering::SeqCst};

type ListenerImpl = PipeListener<Bytes, Bytes>;

/// Wrapper around [`PipeListener`] that implements
/// [`Listener`](crate::local_socket::traits::Listener).
#[derive(Debug)]
pub struct Listener {
	listener: ListenerImpl,
	nonblocking: AtomicEnum<ListenerNonblockingMode>,
}
impl Sealed for Listener {}

impl traits::Listener for Listener {
	type Stream = Stream;

	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		let mut impl_options = PipeListenerOptions::new();
		let NameInner::NamedPipe(path) = options.name.0;
		impl_options.path = path;
		impl_options.nonblocking = options.nonblocking.accept_nonblocking();
		impl_options.security_descriptor = options.security_descriptor;

		Ok(Self {
			listener: impl_options.create()?,
			nonblocking: AtomicEnum::new(options.nonblocking),
		})
	}
	fn accept(&self) -> io::Result<Stream> {
		use ListenerNonblockingMode as LNM;
		let stream = self.listener.accept().map(Stream)?;
		// TODO(2.3.0) verify necessity of orderings
		let nonblocking = self.nonblocking.load(SeqCst);
		if matches!(nonblocking, LNM::Accept) {
			stream.set_nonblocking(false)?;
		} else if matches!(nonblocking, LNM::Stream) {
			stream.set_nonblocking(true)?;
		}
		Ok(stream)
	}
	fn set_nonblocking(&self, nonblocking: ListenerNonblockingMode) -> io::Result<()> {
		self.listener
			.set_nonblocking(nonblocking.accept_nonblocking())?;
		self.nonblocking.store(nonblocking, SeqCst);
		Ok(())
	}
	fn do_not_reclaim_name_on_drop(&mut self) {}
}
impl Iterator for Listener {
	type Item = io::Result<Stream>;
	#[inline(always)]
	fn next(&mut self) -> Option<Self::Item> {
		Some(traits::Listener::accept(self))
	}
}
impl FusedIterator for Listener {}

impl From<Listener> for OwnedHandle {
	#[inline]
	fn from(l: Listener) -> Self {
		l.listener.into()
	}
}
