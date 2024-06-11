use crate::{
	error::{FromHandleError, ReuniteError},
	local_socket::{
		traits::{self, ReuniteResult},
		Name, NameInner,
	},
	os::windows::named_pipe::{
		pipe_mode::Bytes, DuplexPipeStream, RecvPipeStream, SendPipeStream,
	},
	Sealed,
};
use std::{
	io::{self, Write},
	os::windows::io::OwnedHandle,
};

type StreamImpl = DuplexPipeStream<Bytes>;
type RecvHalfImpl = RecvPipeStream<Bytes>;
type SendHalfImpl = SendPipeStream<Bytes>;

/// Wrapper around [`DuplexPipeStream`] that implements
/// [`Stream`](crate::local_socket::traits::Stream).
#[derive(Debug)]
pub struct Stream(pub(super) StreamImpl);

impl Sealed for Stream {}
impl traits::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	fn connect(name: Name<'_>) -> io::Result<Self> {
		let NameInner::NamedPipe(path) = name.0;
		StreamImpl::connect_by_path(path).map(Self)
	}

	#[inline]
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		self.0.set_nonblocking(nonblocking)
	}
	#[inline]
	fn split(self) -> (RecvHalf, SendHalf) {
		let (rh, sh) = self.0.split();
		(RecvHalf(rh), SendHalf(sh))
	}
	fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult<Self> {
		StreamImpl::reunite(rh.0, sh.0)
			.map(Self)
			.map_err(|ReuniteError { rh, sh }| ReuniteError {
				rh: RecvHalf(rh),
				sh: SendHalf(sh),
			})
	}
}

/// Flushing fails with [`Unsupported`](io::ErrorKind::Unsupported).
impl Write for &Stream {
	#[inline]
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		(&self.0).write(buf)
	}
	#[inline]
	fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
		(&self.0).write_vectored(bufs)
	}

	#[inline]
	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
	// FUTURE is_write_vectored
}

impl From<Stream> for OwnedHandle {
	fn from(s: Stream) -> Self {
		// The outer local socket interface has receive and send halves and is always duplex in the
		// unsplit type, so a split pipe stream can never appear here.
		s.0.try_into()
			.expect("split named pipe stream inside `local_socket::Stream`")
	}
}

impl TryFrom<OwnedHandle> for Stream {
	type Error = FromHandleError;

	fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
		match StreamImpl::try_from(handle) {
			Ok(s) => Ok(Self(s)),
			Err(e) => Err(FromHandleError {
				details: Default::default(),
				cause: Some(e.details.into()),
				source: e.source,
			}),
		}
	}
}

multimacro! {
	Stream,
	forward_rbv(StreamImpl, &),
	forward_sync_read,
	forward_sync_ref_read,
	forward_as_handle,
	forward_try_clone,
	derive_sync_mut_write,
	derive_trivial_conv(StreamImpl),
}

/// Wrapper around [`RecvPipeStream`] that implements
/// [`RecvHalf`](crate::local_socket::traits::RecvHalf).
pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
	RecvHalf,
	forward_rbv(RecvHalfImpl, &),
	forward_sync_read,
	forward_sync_ref_read,
	forward_as_handle,
	forward_debug("local_socket::RecvHalf"),
	derive_trivial_conv(RecvHalfImpl),
}

/// Wrapper around [`SendPipeStream`] that implements
/// [`SendHalf`](crate::local_socket::traits::SendHalf).
pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
	SendHalf,
	forward_as_handle,
	forward_debug("local_socket::SendHalf"),
	derive_sync_mut_write,
	derive_trivial_conv(SendHalfImpl),
}

/// Flushing fails with [`Unsupported`](io::ErrorKind::Unsupported).
impl Write for &SendHalf {
	#[inline]
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		(&self.0).write(buf)
	}
	#[inline]
	fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
		(&self.0).write_vectored(bufs)
	}

	#[inline]
	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
	// FUTURE is_write_vectored
}

impl Sealed for RecvHalf {}
impl traits::RecvHalf for RecvHalf {
	type Stream = Stream;
}
impl Sealed for SendHalf {}
impl traits::SendHalf for SendHalf {
	type Stream = Stream;
}
