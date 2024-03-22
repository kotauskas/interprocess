use crate::{
	local_socket::{
		traits::{self, ReuniteResult},
		Name,
	},
	os::windows::named_pipe::{pipe_mode::Bytes, DuplexPipeStream, RecvPipeStream, SendPipeStream},
	Sealed,
};
use std::io;

pub type Stream = DuplexPipeStream<Bytes>;
pub type RecvHalf = RecvPipeStream<Bytes>;
pub type SendHalf = SendPipeStream<Bytes>;

impl Sealed for Stream {}
impl traits::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	fn connect(name: Name<'_>) -> io::Result<Self> {
		if name.is_namespaced() {
			Stream::connect_with_prepend(name.raw(), None)
		} else {
			Stream::connect_by_path(name.raw())
		}
	}

	forward_to_self!(
		fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
		fn split(self) -> (Self::RecvHalf, Self::SendHalf);
		fn reunite(rh: Self::RecvHalf, sh: Self::SendHalf) -> ReuniteResult<Self>;
	);
}
impl Sealed for RecvHalf {}
impl traits::RecvHalf for RecvHalf {
	type Stream = Stream;
}
impl Sealed for SendHalf {}
impl traits::SendHalf for SendHalf {
	type Stream = Stream;
}

// TODO reintroduce shim types to forbid flushes (should fail with NotSupported on all platforms)
