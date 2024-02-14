use super::stream::Stream;
use crate::{
	local_socket::Name,
	os::windows::named_pipe::{
		pipe_mode::Bytes, PipeListener as GenericPipeListener, PipeListenerOptions,
	},
};
use std::{
	io,
	path::{Path, PathBuf},
};

type PipeListener = GenericPipeListener<Bytes, Bytes>;

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
		options.create().map(Self)
	}
	pub fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept()?;
		Ok(Stream(inner))
	}
	pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		self.0.set_nonblocking(nonblocking)
	}
	pub fn do_not_reclaim_name_on_drop(&mut self) {}
}
forward_into_handle!(Listener);
