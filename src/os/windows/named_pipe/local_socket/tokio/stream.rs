use crate::{
	error::{FromHandleError, ReuniteError},
	local_socket::{
		traits::tokio::{self as traits, ReuniteResult},
		Name, NameInner,
	},
	os::windows::named_pipe::{
		pipe_mode::Bytes,
		tokio::{DuplexPipeStream, RecvPipeStream, SendPipeStream},
	},
	Sealed,
};
use std::{
	io,
	os::windows::prelude::*,
	pin::Pin,
	task::{Context, Poll},
};
use tokio::io::AsyncWrite;

type StreamImpl = DuplexPipeStream<Bytes>;
type RecvHalfImpl = RecvPipeStream<Bytes>;
type SendHalfImpl = SendPipeStream<Bytes>;

#[derive(Debug)]
pub struct Stream(pub(super) StreamImpl);
impl Sealed for Stream {}
impl traits::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	async fn connect(name: Name<'_>) -> io::Result<Self> {
		let NameInner::NamedPipe(path) = name.0;
		StreamImpl::connect_by_path(path).await.map(Self)
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

impl AsyncWrite for &Stream {
	#[inline]
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut &self.get_mut().0).poll_write(cx, buf)
	}
	#[inline]
	fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
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
	pinproj_for_unpin(StreamImpl),
	forward_rbv(StreamImpl, &),
	forward_tokio_read,
	forward_tokio_ref_read,
	forward_as_handle,
	derive_tokio_mut_write,
	derive_trivial_conv(StreamImpl),
}

pub struct RecvHalf(pub(super) RecvHalfImpl);
impl Sealed for RecvHalf {}
impl traits::RecvHalf for RecvHalf {
	type Stream = Stream;
}
multimacro! {
	RecvHalf,
	pinproj_for_unpin(RecvHalfImpl),
	forward_rbv(RecvHalfImpl, &),
	forward_tokio_read,
	forward_tokio_ref_read,
	forward_as_handle,
	forward_debug("local_socket::RecvHalf"),
	derive_trivial_conv(RecvHalfImpl),
}

pub struct SendHalf(pub(super) SendHalfImpl);
impl Sealed for SendHalf {}
impl traits::SendHalf for SendHalf {
	type Stream = Stream;
}

impl AsyncWrite for &SendHalf {
	#[inline]
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut &self.get_mut().0).poll_write(cx, buf)
	}
	#[inline]
	fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}
}

multimacro! {
	SendHalf,
	forward_rbv(SendHalfImpl, &),
	forward_as_handle,
	forward_debug("local_socket::SendHalf"),
	derive_tokio_mut_write,
	derive_trivial_conv(SendHalfImpl),
}
