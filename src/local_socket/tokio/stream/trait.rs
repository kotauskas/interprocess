#![allow(private_bounds)]

use crate::{
	bound_util::{RefTokioAsyncRead, RefTokioAsyncWrite},
	local_socket::Name,
	Sealed,
};
use std::{future::Future, io};
use tokio::io::{AsyncRead, AsyncWrite};

/// Tokio local socket stream implementations.
///
/// Types on which this trait is implemented are variants of the
/// [`Stream` enum](super::enum::Stream). In addition, it is implemented on `Stream` itself, which
/// makes it a trait object of sorts. See its documentation for more on the semantics of the methods
/// seen here.
pub trait Stream:
	AsyncRead + RefTokioAsyncRead + AsyncWrite + RefTokioAsyncWrite + Send + Sync + Sized + Sealed
{
	/// Receive half type returned by [`.split()`](Stream::split).
	type RecvHalf: RecvHalf<Stream = Self>;
	/// Send half type returned by [`.split()`](Stream::split).
	type SendHalf: SendHalf<Stream = Self>;

	/// Asynchronously connects to a remote local socket server.
	fn connect(name: Name<'_>) -> impl Future<Output = io::Result<Self>> + Send + Sync;

	/// Splits a stream into a receive half and a send half, which can be used to receive from and
	/// send to the stream concurrently from different Tokio tasks, entailing a memory allocation.
	fn split(self) -> (Self::RecvHalf, Self::SendHalf);

	/// Attempts to reunite a receive half with a send half to yield the original stream back,
	/// returning both halves as an error if they belong to different streams (or when using this
	/// method on streams that haven't been split to begin with).
	fn reunite(rh: Self::RecvHalf, sh: Self::SendHalf) -> ReuniteResult<Self>;
}

/// Receive halves of Tokio [`Stream`]s, obtained through [`.split()`](Stream::split).
///
/// Types on which this trait is implemented are variants of the
/// [`RecvHalf` enum](super::enum::RecvHalf). In addition, it is implemented on `RecvHalf` itself,
/// which makes it a trait object of sorts.
pub trait RecvHalf: Sized + AsyncRead + RefTokioAsyncRead + Sealed {
	/// The stream type the half is split from.
	type Stream: Stream;
}

/// Send halves of Tokio [`Stream`]s, obtained through [`.split()`](Stream::split).
///
/// Types on which this trait is implemented are variants of the
/// [`SendHalf` enum](super::enum::SendHalf). In addition, it is implemented on `SendHalf` itself,
/// which makes it a trait object of sorts.
pub trait SendHalf: Sized + AsyncWrite + RefTokioAsyncWrite + Sealed {
	/// The stream type the half is split from.
	type Stream: Stream;
}

/// [`ReuniteResult`](crate::error::ReuniteResult) for the [Tokio `Stream` trait](Stream).
pub type ReuniteResult<S> =
	crate::error::ReuniteResult<S, <S as Stream>::RecvHalf, <S as Stream>::SendHalf>;
