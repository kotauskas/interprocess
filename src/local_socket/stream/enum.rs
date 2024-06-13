use super::r#trait;
use crate::{local_socket::Name, TryClone};
use std::io::{self, prelude::*, IoSlice, IoSliceMut};

#[cfg(unix)]
use crate::os::unix::uds_local_socket as uds_impl;
#[cfg(windows)]
use crate::os::windows::named_pipe::local_socket as np_impl;

impmod! {local_socket::dispatch_sync}

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
			Ok(())
		}
		#[inline]
		fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
			dispatch!($ty: x in self => x.write_vectored(bufs))
		}
	};
	($ty:ident) => {
		/// Flushing is an always successful no-op.
		impl Write for &$ty {
			dispatch_write!(@iw $ty);
		}
		/// Flushing is an always successful no-op.
		impl Write for $ty {
			dispatch_write!(@iw $ty);
		}
	};
}

mkenum!(
/// Local socket byte stream, obtained either from [`Listener`](super::super::Listener) or by
/// connecting to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
#[doc = doctest_file::include_doctest!("examples/local_socket/sync/stream.rs")]
/// ```
Stream);
impl r#trait::Stream for Stream {
	type RecvHalf = RecvHalf;
	type SendHalf = SendHalf;

	#[inline]
	fn connect(name: Name<'_>) -> io::Result<Self> {
		dispatch_sync::connect(name)
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
multimacro! {
	Stream,
	dispatch_read,
	dispatch_write,
}

mkenum!(
/// Receive half of a local socket stream, obtained by splitting a [`Stream`].
"local_socket::" RecvHalf);
impl r#trait::RecvHalf for RecvHalf {
	type Stream = Stream;
}
dispatch_read!(RecvHalf);

mkenum!(
/// Send half of a local socket stream, obtained by splitting a [`Stream`].
"local_socket::" SendHalf);
impl r#trait::SendHalf for SendHalf {
	type Stream = Stream;
}
dispatch_write!(SendHalf);

/// [`ReuniteError`](crate::error::ReuniteError) for [`Stream`].
pub type ReuniteError = crate::error::ReuniteError<RecvHalf, SendHalf>;

/// Result type for [`.reunite()`](trait::Stream::reunite) on [`Stream`].
pub type ReuniteResult = r#trait::ReuniteResult<Stream>;
