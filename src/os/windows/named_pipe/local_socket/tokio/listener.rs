use super::Stream;
use crate::{
	local_socket::Name,
	os::windows::{
		named_pipe::{
			pipe_mode,
			tokio::{PipeListener as GenericPipeListener, PipeListenerOptionsExt as _},
			PipeListenerOptions,
		},
		path_conversion::*,
	},
};
use std::{io, path::Path};

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct Listener(PipeListener);
impl Listener {
	pub fn bind(name: Name<'_>, _: bool) -> io::Result<Self> {
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
	pub async fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept().await?;
		Ok(Stream(inner))
	}
	pub fn do_not_reclaim_name_on_drop(&mut self) {}
}
