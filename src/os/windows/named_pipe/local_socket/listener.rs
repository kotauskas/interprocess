use super::stream::Stream;
use crate::{
	local_socket::{traits, Name},
	os::windows::named_pipe::{pipe_mode::Bytes, PipeListener, PipeListenerOptions},
};
use std::{
	io,
	path::{Path, PathBuf},
};

type ListenerImpl = PipeListener<Bytes, Bytes>;

/// Wrapper around [`PipeListener`] that implements
/// [`Listener`](crate::local_socket::traits::Listener).
#[derive(Debug)]
pub struct Listener(ListenerImpl);
#[doc(hidden)]
impl crate::Sealed for Listener {}
impl traits::Listener for Listener {
	type Stream = Stream;

	#[inline]
	fn bind(name: Name<'_>) -> io::Result<Self> {
		Self::bind_without_name_reclamation(name)
	}
	fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Self> {
		let path = Path::new(name.raw());
		let mut options = PipeListenerOptions::new();
		options.path = if name.is_namespaced() {
			// PERF this allocates twice
			[Path::new(r"\\.\pipe\"), path]
				.iter()
				.collect::<PathBuf>()
				.into()
		} else {
			path.into()
		};
		options.create().map(Self)
	}
	fn accept(&self) -> io::Result<Stream> {
		self.0.accept()
	}
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		self.0.set_nonblocking(nonblocking)
	}
	fn do_not_reclaim_name_on_drop(&mut self) {}
}
forward_into_handle!(Listener);
