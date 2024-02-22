use crate::{
	error::{FromHandleError, ReuniteError},
	local_socket::{
		traits::{self, ReuniteResult},
		Name,
	},
	os::windows::named_pipe::{pipe_mode::Bytes, DuplexPipeStream, RecvPipeStream, SendPipeStream},
	Sealed,
};
use std::{io, os::windows::prelude::*};

type StreamImpl = DuplexPipeStream<Bytes>;
type RecvHalfImpl = RecvPipeStream<Bytes>;
type SendHalfImpl = SendPipeStream<Bytes>;

/// Wrapper around [`DuplexPipeStream`] that implements
/// [`Stream`](crate::local_socket::traits::Stream).
#[derive(Debug)]
pub struct Stream(pub(super) StreamImpl);
#[doc(hidden)]
impl Sealed for Stream {}
impl traits::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	fn connect(name: Name<'_>) -> io::Result<Self> {
		if name.is_namespaced() {
			StreamImpl::connect_with_prepend(name.raw(), None)
		} else {
			StreamImpl::connect(name.raw())
		}
		.map(Self)
	}
	#[inline]
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		self.0.set_nonblocking(nonblocking)
	}
	#[inline]
	fn split(self) -> (RecvHalf, SendHalf) {
		let (r, w) = self.0.split();
		(RecvHalf(r), SendHalf(w))
	}
	#[inline]
	fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult<Self> {
		StreamImpl::reunite(rh.0, sh.0)
			.map(Self)
			.map_err(|ReuniteError { rh, sh }| ReuniteError {
				rh: RecvHalf(rh),
				sh: SendHalf(sh),
			})
	}
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
	forward_sync_ref_rw, // The thunking already happens inside.
	forward_as_handle,
	forward_try_clone,
	derive_sync_mut_rw,
}

/// Wrapper around [`RecvPipeStream`] that implements
/// [`RecvHalf`](crate::local_socket::traits::RecvHalf).
#[derive(Debug)]
pub struct RecvHalf(pub(super) RecvHalfImpl);
#[doc(hidden)]
impl Sealed for RecvHalf {}
impl traits::RecvHalf for RecvHalf {
	type Stream = Stream;
}
multimacro! {
	RecvHalf,
	forward_rbv(RecvHalfImpl, &),
	forward_sync_ref_read,
	forward_as_handle,
	derive_sync_mut_read,
}

/// Wrapper around [`SendPipeStream`] that implements
/// [`SendHalf`](crate::local_socket::traits::SendHalf).
#[derive(Debug)]
pub struct SendHalf(pub(super) SendHalfImpl);
#[doc(hidden)]
impl Sealed for SendHalf {}
impl traits::SendHalf for SendHalf {
	type Stream = Stream;
}
multimacro! {
	SendHalf,
	forward_rbv(SendHalfImpl, &),
	forward_sync_ref_write,
	forward_as_handle,
	derive_sync_mut_write,
}
