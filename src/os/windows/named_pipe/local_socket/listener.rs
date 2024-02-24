use super::stream::Stream;
use crate::{
	local_socket::{traits, Name},
	os::windows::{
		named_pipe::{pipe_mode::Bytes, PipeListener, PipeListenerOptions},
		path_conversion::*,
	},
};
use std::{io, path::Path};

type ListenerImpl = PipeListener<Bytes, Bytes>;

/// Wrapper around [`PipeListener`] that implements
/// [`Listener`](crate::local_socket::traits::Listener).
#[derive(Debug)]
pub struct Listener(ListenerImpl);
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
