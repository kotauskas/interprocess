use super::Stream;
use crate::{
	local_socket::{traits::tokio as traits, ListenerOptions},
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
use std::{borrow::Cow, io, path::Path};

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct Listener(PipeListener);
impl Sealed for Listener {}
impl traits::Listener for Listener {
	type Stream = Stream;

	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		let mut impl_options = PipeListenerOptions::new();
		impl_options.path = if options.name.is_path() {
			Path::new(options.name.raw())
				.to_wtf_16()
				.map_err(to_io_error)?
		} else {
			convert_and_encode_path(options.name.raw(), None).map(Cow::Owned)?
		};
		impl_options.security_descriptor = options.security_descriptor;
		impl_options.create_tokio().map(Self)
	}
	async fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept().await?;
		Ok(Stream(inner))
	}
	fn do_not_reclaim_name_on_drop(&mut self) {}
}
