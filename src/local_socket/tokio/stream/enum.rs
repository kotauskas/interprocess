use super::r#trait;
use crate::local_socket::{async_flush_unsupported, Name};
use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
#[cfg(unix)]
use {crate::os::unix::uds_local_socket::tokio as uds_impl, std::os::unix::prelude::*};
#[cfg(windows)]
use {
	crate::os::windows::named_pipe::local_socket::tokio as np_impl, std::os::windows::prelude::*,
};

impmod! {local_socket::dispatch_tokio,
	self as dispatch,
}

macro_rules! dispatch_read {
	(@iw $ty:ident) => {
		#[inline]
		fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
			dispatch!($ty: x in self.get_mut() => Pin::new(x).poll_read(cx, buf))
		}
	};
	($ty:ident) => {
		impl AsyncRead for &$ty {
			dispatch_read!(@iw $ty);
		}
		impl AsyncRead for $ty {
			dispatch_read!(@iw $ty);
		}
	};
}
macro_rules! dispatch_write {
	(@iw $ty:ident) => {
		#[inline]
		fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
			dispatch!($ty: x in self.get_mut() => Pin::new(x).poll_write(cx, buf))
		}
		#[inline]
		fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
			async_flush_unsupported()
		}
		#[inline]
		fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
			dispatch!($ty: x in self.get_mut() => Pin::new(x).poll_shutdown(cx))
		}
	};
	($ty:ident) => {
		impl AsyncWrite for &$ty {
			dispatch_write!(@iw $ty);
		}
		impl AsyncWrite for $ty {
			dispatch_write!(@iw $ty);
		}
	};
}

mkenum!(
/// Tokio-based local socket byte stream, obtained eiter from [`Listener`](super::Listener) or by
/// connecting to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::local_socket::{
/// 	tokio::{prelude::*, Stream},
/// 	NameTypeSupport, ToFsName, ToNsName,
/// };
/// use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, try_join};
///
/// // Pick a name. There isn't a helper function for this, mostly because it's largely unnecessary:
/// // in Rust, `match` is your concise, readable and expressive decision making construct.
/// let name = {
/// 	// This scoping trick allows us to nicely contain the import inside the `match`, so that if
/// 	// any imports of variants named `Both` happen down the line, they won't collide with the
/// 	// enum we're working with here. Maybe someone should make a macro for this.
/// 	use NameTypeSupport::*;
/// 	match NameTypeSupport::query() {
/// 		OnlyFs => "/tmp/example.sock".to_fs_name()?,
/// 		OnlyNs | Both => "example.sock".to_ns_name()?,
/// 	}
/// };
///
/// // Await this here since we can't do a whole lot without a connection.
/// let conn = Stream::connect(name).await?;
///
/// // This consumes our connection and splits it into two halves,
/// // so that we can concurrently use both.
/// let (recver, mut sender) = conn.split();
/// let mut recver = BufReader::new(recver);
///
/// // Allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Describe the send operation as writing our whole string.
/// let send = sender.write_all(b"Hello from client!\n");
/// // Describe the receive operation as receiving until a newline into our buffer.
/// let recv = recver.read_line(&mut buffer);
///
/// // Concurrently perform both operations.
/// try_join!(send, recv)?;
///
/// // Close the connection a bit earlier than you'd think we would. Nice practice!
/// drop((recver, sender));
///
/// // Display the results when we're done!
/// println!("Server answered: {}", buffer.trim());
/// # Ok(()) }
/// ```
Stream);

impl r#trait::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	#[inline]
	async fn connect(name: Name<'_>) -> io::Result<Self> {
		dispatch::connect(name).await
	}
	fn split(self) -> (RecvHalf, SendHalf) {
		match self {
			#[cfg(windows)]
			Stream::NamedPipe(s) => {
				let (rh, sh) = s.split();
				(RecvHalf::NamedPipe(rh), SendHalf::NamedPipe(sh))
			}
			#[cfg(unix)]
			Stream::UdSocket(s) => {
				let (rh, sh) = s.split();
				(RecvHalf::UdSocket(rh), SendHalf::UdSocket(sh))
			}
		}
	}
	fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult {
		match (rh, sh) {
			#[cfg(windows)]
			(RecvHalf::NamedPipe(rh), SendHalf::NamedPipe(sh)) => np_impl::Stream::reunite(rh, sh)
				.map(From::from)
				.map_err(|e| e.convert_halves()),
			#[cfg(unix)]
			(RecvHalf::UdSocket(rh), SendHalf::UdSocket(sh)) => uds_impl::Stream::reunite(rh, sh)
				.map(From::from)
				.map_err(|e| e.convert_halves()),
			#[allow(unreachable_patterns)]
			(rh, sh) => Err(ReuniteError { rh, sh }),
		}
	}
}

/// Creates a [`NamedPipe`](Stream::NamedPipe) stream.
#[cfg(windows)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
impl TryFrom<OwnedHandle> for Stream {
	type Error = <np_impl::Stream as TryFrom<OwnedHandle>>::Error;
	#[inline]
	fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
		handle.try_into().map(Self::NamedPipe)
	}
}

/// Creates a [`UdSocket`](Stream::UdSocket) stream.
#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
impl TryFrom<OwnedFd> for Stream {
	type Error = <uds_impl::Stream as TryFrom<OwnedFd>>::Error;
	#[inline]
	fn try_from(fd: OwnedFd) -> Result<Self, Self::Error> {
		fd.try_into().map(Self::UdSocket)
	}
}

multimacro! {
	Stream,
	dispatch_read,
	dispatch_write,
	dispatch_as_handle,
}

mkenum!(
/// Receive half of a Tokio-based local socket stream, obtained by splitting a [`Stream`].
RecvHalf);
impl r#trait::RecvHalf for RecvHalf {
	type Stream = Stream;
}
multimacro! {
	RecvHalf,
	dispatch_read,
	dispatch_as_handle,
}

mkenum!(
/// Send half of a Tokio-based local socket stream, obtained by splitting a [`Stream`].
SendHalf);
impl r#trait::SendHalf for SendHalf {
	type Stream = Stream;
}
multimacro! {
	SendHalf,
	dispatch_write,
	dispatch_as_handle,
}

/// [`ReuniteError`](crate::error::ReuniteError) for [`Stream`].
pub type ReuniteError = crate::error::ReuniteError<RecvHalf, SendHalf>;

/// Result type for [`.reunite()`](r#trait::Stream::reunite) on [`Stream`].
pub type ReuniteResult = r#trait::ReuniteResult<Stream>;
