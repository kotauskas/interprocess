use super::stream::Stream;
use crate::{
	local_socket::{
		traits::{self, ListenerNonblockingMode, Stream as _},
		ListenerOptions,
	},
	os::windows::{
		named_pipe::{pipe_mode::Bytes, PipeListener, PipeListenerOptions},
		path_conversion::*,
	},
	AtomicEnum,
};
use std::{io, os::windows::prelude::*, path::Path, sync::atomic::Ordering::SeqCst};

type ListenerImpl = PipeListener<Bytes, Bytes>;

/// Wrapper around [`PipeListener`] that implements
/// [`Listener`](crate::local_socket::traits::Listener).
#[derive(Debug)]
pub struct Listener {
	listener: ListenerImpl,
	nonblocking: AtomicEnum<ListenerNonblockingMode>,
}
impl crate::Sealed for Listener {}

impl traits::Listener for Listener {
	type Stream = Stream;

	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		let mut impl_options = PipeListenerOptions::new();
		impl_options.path = if options.name.is_path() {
			Path::new(options.name.raw())
				.to_wtf_16()
				.map_err(to_io_error)?
		} else {
			convert_and_encode_path(options.name.raw(), None)
				.to_wtf_16()
				.map_err(to_io_error)?
		};
		impl_options.nonblocking = options.nonblocking.accept_nonblocking();

		Ok(Self {
			listener: impl_options.create()?,
			nonblocking: AtomicEnum::new(options.nonblocking),
		})
	}
	fn accept(&self) -> io::Result<Stream> {
		use ListenerNonblockingMode as LNM;
		let stream = self.listener.accept().map(Stream)?;
		// TODO verify necessity of orderings
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

impl From<Listener> for OwnedHandle {
	#[inline]
	fn from(l: Listener) -> Self {
		l.listener.into()
	}
}
