#![allow(private_bounds)]

use crate::{
	bound_util::{RefRead, RefWrite},
	local_socket::Name,
	Sealed,
};
use std::io::{self, prelude::*};

/// Local socket stream implementations.
///
/// Types on which this trait is implemented are variants of the
/// [`Stream` enum](super::enum::Stream). In addition, it is implemented on `Stream` itself, which
/// makes it a trait object of sorts. See its documentation for more on the semantics of the methods
/// seen here.
pub trait Stream: Read + RefRead + Write + RefWrite + Send + Sync + Sized + Sealed {
	/// Receive half type returned by [`.split()`](Stream::split).
	type RecvHalf: RecvHalf<Stream = Self>;
	/// Send half type returned by [`.split()`](Stream::split).
	type SendHalf: SendHalf<Stream = Self>;

	/// Connects to a remote local socket server.
	fn connect(name: Name<'_>) -> io::Result<Self>;

	/// Enables or disables the nonblocking mode for the stream. By default, it is disabled.
	///
	/// In nonblocking mode, receiving and sending immediately returns with the
	/// [`WouldBlock`](io::ErrorKind::WouldBlock) error in situations when they would normally block
	/// for an uncontrolled amount of time. The specific situations are:
	/// -	Receiving is attempted and there is no new data available;
	/// -	Sending is attempted and the buffer is full due to the other side not yet having
	/// 	received previously sent data.
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;

	/// Splits a stream into a receive half and a send half, which can be used to receive from and
	/// send to the stream concurrently from different threads, entailing a memory allocation.
	fn split(self) -> (Self::RecvHalf, Self::SendHalf);

	/// Attempts to reunite a receive half with a send half to yield the original stream back,
	/// returning both halves as an error if they belong to different streams (or when using this
	/// method on streams that haven't been split to begin with).
	fn reunite(rh: Self::RecvHalf, sh: Self::SendHalf) -> ReuniteResult<Self>;

	// Do not add methods to this trait that aren't directly tied to non-async streams. A new trait,
	// which should be called StreamExtra or StreamCommon or something along those lines, is to be
	// created for features like impersonation (ones that are instantaneous in nature).
}

/// Receive halves of [`Stream`]s, obtained through [`.split()`](Stream::split).
///
/// Types on which this trait is implemented are variants of the
/// [`RecvHalf` enum](super::enum::RecvHalf). In addition, it is implemented on `RecvHalf` itself,
/// which makes it a trait object of sorts.
pub trait RecvHalf: Sized + Read + RefRead + Sealed {
	/// The stream type the half is split from.
	type Stream: Stream;
}

/// Send halves of [`Stream`]s, obtained through [`.split()`](Stream::split).
///
/// Types on which this trait is implemented are variants of the
/// [`SendHalf` enum](super::enum::SendHalf). In addition, it is implemented on `SendHalf` itself,
/// which makes it a trait object of sorts.
pub trait SendHalf: Sized + Write + RefWrite + Sealed {
	/// The stream type the half is split from.
	type Stream: Stream;
}

/// [`ReuniteResult`](crate::error::ReuniteResult) for the [`Stream` trait](Stream).
pub type ReuniteResult<S> =
	crate::error::ReuniteResult<S, <S as Stream>::RecvHalf, <S as Stream>::SendHalf>;
