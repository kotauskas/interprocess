use super::stream::Stream;
use crate::{
	local_socket::{
		traits::{self, ListenerNonblockingMode, Stream as _},
		Name,
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

	#[inline]
	fn bind(name: Name<'_>) -> io::Result<Self> {
		Self::bind_without_name_reclamation(name)
	}
	fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Self> {
		let mut options = PipeListenerOptions::new();
		options.path = if name.is_path() {
			Path::new(name.raw()).to_wtf_16().map_err(to_io_error)?
		} else {
			convert_and_encode_path(name.raw(), None)
				.to_wtf_16()
				.map_err(to_io_error)?
		};
		let listener = options.create()?;
		Ok(Self {
			listener,
			nonblocking: AtomicEnum::new(ListenerNonblockingMode::Neither),
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
		use ListenerNonblockingMode::*;
		self.listener
			.set_nonblocking(matches!(nonblocking, Accept | Both))?;
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
