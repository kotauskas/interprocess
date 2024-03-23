use super::Stream;
use crate::{
	local_socket::{traits::tokio as traits, Name},
	os::windows::{
		named_pipe::{
			pipe_mode,
			tokio::{PipeListener as GenericPipeListener, PipeListenerOptionsExt as _},
			PipeListenerOptions,
		},
		path_conversion::*,
	},
	Sealed,
};
use std::{io, path::Path};

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct Listener(PipeListener);
impl Sealed for Listener {}
impl traits::Listener for Listener {
	type Stream = Stream;
	fn bind(name: Name<'_>) -> io::Result<Self> {
		let mut options = PipeListenerOptions::new();
		options.path = if name.is_path() {
			Path::new(name.raw()).to_wtf_16().map_err(to_io_error)?
		} else {
			convert_and_encode_path(name.raw(), None)
				.to_wtf_16()
				.map_err(to_io_error)?
		};
		options.create_tokio().map(Self)
	}
	#[inline]
	fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Self> {
		Self::bind(name)
	}
	async fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept().await?;
		Ok(Stream(inner))
	}
	fn do_not_reclaim_name_on_drop(&mut self) {}
}
