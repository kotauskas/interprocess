use super::super::Name;
use std::io;

impmod! {local_socket::tokio,
	Stream as StreamImpl,
	RecvHalf as RecvHalfImpl,
	SendHalf as SendHalfImpl,
}

/// Tokio-based local socket byte stream, obtained eiter from [`Listener`](super::Listener) or by
/// connecting to an existing local socket.
///
/// # Examples
///
/// ## Basic client
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use interprocess::local_socket::{tokio::Stream, NameTypeSupport, ToFsName, ToNsName};
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
pub struct Stream(pub(super) StreamImpl);
impl Stream {
	/// Connects to a remote local socket server.
	#[inline]
	pub async fn connect(name: Name<'_>) -> io::Result<Self> {
		StreamImpl::connect(name).await.map(Self::from)
	}

	/// Splits a stream into a receive half and a send half, which can be used to receive data from
	/// and send data to the stream concurrently from independently spawned tasks, entailing a
	/// memory allocation.
	#[inline]
	pub fn split(self) -> (RecvHalf, SendHalf) {
		let (r, w) = self.0.split();
		(RecvHalf(r), SendHalf(w))
	}
	/// Attempts to reunite a receive half with a send half to yield the original stream back,
	/// returning both halves as an error if they belong to different streams (or when using
	/// this method on streams that haven't been split to begin with).
	#[inline]
	pub fn reunite(rh: RecvHalf, sh: SendHalf) -> ReuniteResult {
		StreamImpl::reunite(rh.0, sh.0).map(Self).map_err(
			|crate::error::ReuniteError { rh, sh }| ReuniteError {
				rh: RecvHalf(rh),
				sh: SendHalf(sh),
			},
		)
	}
}
#[doc(hidden)]
impl From<StreamImpl> for Stream {
	#[inline]
	fn from(inner: StreamImpl) -> Self {
		Self(inner)
	}
}

multimacro! {
	Stream,
	pinproj_for_unpin(StreamImpl),
	forward_rbv(StreamImpl, &),
	forward_tokio_rw,
	forward_tokio_ref_rw,
	forward_as_handle,
	forward_try_from_handle(StreamImpl),
	forward_debug,
	derive_asraw,
}

/// Receive half of a Tokio-based local socket stream, obtained by splitting a [`Stream`].
///
/// # Examples
// TODO
pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
	RecvHalf,
	pinproj_for_unpin(RecvHalfImpl),
	forward_rbv(RecvHalfImpl, &),
	forward_tokio_read,
	forward_tokio_ref_read,
	forward_as_handle,
	forward_debug,
	derive_asraw,
}
/// Send half of a Tokio-based local socket stream, obtained by splitting a [`Stream`].
///
/// # Examples
// TODO
pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
	SendHalf,
	pinproj_for_unpin(SendHalfImpl),
	forward_rbv(SendHalfImpl, &),
	forward_tokio_write,
	forward_tokio_ref_write,
	forward_as_handle,
	forward_debug,
	derive_asraw,
}

/// [`ReuniteError`](crate::error::ReuniteError) for Tokio local socket streams.
pub type ReuniteError = crate::error::ReuniteError<RecvHalf, SendHalf>;
/// Result type for [`Stream::reunite()`].
pub type ReuniteResult = Result<Stream, ReuniteError>;
