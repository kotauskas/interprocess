use super::Stream;
use crate::{
	local_socket::Name,
	os::windows::named_pipe::{
		pipe_mode,
		tokio::{PipeListener as GenericPipeListener, PipeListenerOptionsExt as _},
		PipeListenerOptions,
	},
};
use std::{
	io,
	path::{Path, PathBuf},
};

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct Listener(PipeListener);
impl Listener {
	pub fn bind(name: Name<'_>, _: bool) -> io::Result<Self> {
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
		options.create_tokio().map(Self)
	}
	pub async fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept().await?;
		Ok(Stream(inner))
	}
	pub fn do_not_reclaim_name_on_drop(&mut self) {}
}
