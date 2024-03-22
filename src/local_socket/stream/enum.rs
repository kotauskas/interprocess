use super::r#trait;
use crate::{
	local_socket::{flush_unsupported, Name},
	TryClone,
};
use std::io::{self, prelude::*, IoSlice, IoSliceMut};
#[cfg(unix)]
use {crate::os::unix::uds_local_socket as uds_impl, std::os::unix::prelude::*};
#[cfg(windows)]
use {crate::os::windows::named_pipe::local_socket as np_impl, std::os::windows::prelude::*};

// FIXME awkward macro syntax
impmod! {local_socket::dispatch,
	self,
}

macro_rules! dispatch_read {
	(@iw $ty:ident) => {
		#[inline]
		fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
			dispatch!($ty: x in self => x.read(buf))
		}
		#[inline]
		fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
			dispatch!($ty: x in self => x.read_vectored(bufs))
		}
	};
	($ty:ident) => {
		impl Read for &$ty {
			dispatch_read!(@iw $ty);
		}
		impl Read for $ty {
			dispatch_read!(@iw $ty);
		}
	};
}
macro_rules! dispatch_write {
	(@iw $ty:ident) => {
		#[inline]
		fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
			dispatch!($ty: x in self => x.write(buf))
		}
		#[inline]
		fn flush(&mut self) -> io::Result<()> {
			flush_unsupported()
		}
		#[inline]
		fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
			dispatch!($ty: x in self => x.write_vectored(bufs))
		}
	};
	($ty:ident) => {
		impl Write for &$ty {
			dispatch_write!(@iw $ty);
		}
		impl Write for $ty {
			dispatch_write!(@iw $ty);
		}
	};
}

mkenum!(
/// Local socket byte stream, obtained either from [`Listener`](super::Listener) or by connecting
/// to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// use interprocess::local_socket::{prelude::*, Stream, NameTypeSupport, ToFsName, ToNsName};
/// use std::io::{prelude::*, BufReader};
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
/// // Preemptively allocate a sizeable buffer for receiving.
/// // This size should be enough and should be easy to find for the allocator.
/// let mut buffer = String::with_capacity(128);
///
/// // Create our connection. This will block until the server accepts our connection, but will fail
/// // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
/// // where connecting to a port that's not bound to any server will send a "connection refused"
/// // response, but that will take twice the ping, the roundtrip time, to reach the client.
/// let conn = Stream::connect(name)?;
/// // Wrap it into a buffered reader right away so that we could receive a single line out of it.
/// let mut conn = BufReader::new(conn);
///
/// // Send our message into the stream. This will finish either when the whole message has been
/// // sent or if a send operation returns an error. (`.get_mut()` is to get the sender,
/// // `BufReader` doesn't implement pass-through `Write`.)
/// conn.get_mut().write_all(b"Hello from client!\n")?;
///
/// // We now employ the buffer we allocated prior and receive a single line, interpreting a newline
/// // character as an end-of-file (because local sockets cannot be portably shut down), verifying
/// // validity of UTF-8 on the fly.
/// conn.read_line(&mut buffer)?;
///
/// // Print out the result, getting the newline for free!
/// print!("Server answered: {buffer}");
/// # std::io::Result::<()>::Ok(())
/// ```
Stream);
impl r#trait::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	#[inline]
	fn connect(name: Name<'_>) -> io::Result<Self> {
		dispatch::connect(name)
	}
	#[inline]
	fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
		dispatch!(Self: x in self => x.set_nonblocking(nonblocking))
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
impl TryClone for Stream {
	fn try_clone(&self) -> io::Result<Self> {
		dispatch!(Self: x in self => x.try_clone()).map(From::from)
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

#[cfg(windows)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
impl TryFrom<Stream> for OwnedHandle {
	type Error = <OwnedHandle as TryFrom<np_impl::Stream>>::Error;
	fn try_from(slf: Stream) -> Result<Self, Self::Error> {
		match slf {
			Stream::NamedPipe(s) => s.try_into(),
		}
	}
}

/// Creates a [`UdSocket`](Stream::UdSocket) stream.
#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
impl From<OwnedFd> for Stream {
	#[inline]
	fn from(fd: OwnedFd) -> Self {
		Self::UdSocket(fd.into())
	}
}

#[cfg(unix)]
#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
impl From<Stream> for OwnedFd {
	fn from(slf: Stream) -> Self {
		match slf {
			Stream::UdSocket(s) => s.into(),
		}
	}
}

multimacro! {
	Stream,
	dispatch_read,
	dispatch_write,
	dispatch_as_handle,
}

// TODO maybe adjust the Debug of halves to mention that they're local sockets

mkenum!(
/// Receive half of a local socket stream, obtained by splitting a [`Stream`].
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
/// Send half of a local socket stream, obtained by splitting a [`Stream`].
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
