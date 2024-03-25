use super::*;
use crate::os::windows::{
	named_pipe::{NeedsFlushVal, WaitTimeout},
	path_conversion::*,
};
use std::{ffi::OsStr, mem::take, path::Path};

impl RawPipeStream {
	pub(super) fn new(inner: InnerTokio) -> Self {
		Self {
			inner: Some(inner),
			needs_flush: NeedsFlush::from(NeedsFlushVal::No),
			//recv_msg_state: Mutex::new(RecvMsgState::NotRecving),
		}
	}
	pub(crate) fn new_server(server: TokioNPServer) -> Self {
		Self::new(InnerTokio::Server(server))
	}
	fn new_client(client: TokioNPClient) -> Self {
		Self::new(InnerTokio::Client(client))
	}

	async fn wait_for_server(path: Vec<u16>) -> io::Result<Vec<u16>> {
		tokio::task::spawn_blocking(move || {
			c_wrappers::block_for_server(&path, WaitTimeout::DEFAULT)?;
			Ok(path)
		})
		.await
		.expect("waiting for server panicked")
	}

	async fn connect(
		path: &Path,
		recv: Option<PipeMode>,
		send: Option<PipeMode>,
	) -> io::Result<Self> {
		Self::_connect(encode_to_wtf16(path.as_os_str()), recv, send).await
	}

	async fn connect_with_prepend(
		pipename: &OsStr,
		hostname: Option<&OsStr>,
		recv: Option<PipeMode>,
		send: Option<PipeMode>,
	) -> io::Result<Self> {
		Self::_connect(convert_and_encode_path(pipename, hostname), recv, send).await
	}

	async fn _connect(
		mut path: Vec<u16>,
		recv: Option<PipeMode>,
		send: Option<PipeMode>,
	) -> io::Result<Self> {
		let client = loop {
			match c_wrappers::connect_without_waiting(&path, recv, send, true) {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					let path_take = Self::wait_for_server(take(&mut path)).await?;
					path = path_take;
				}
				not_waiting => break not_waiting?,
			}
		};
		let client = unsafe { TokioNPClient::from_raw_handle(client.into_raw_handle())? };
		/* MESSAGE READING DISABLED
		// FIXME should probably upstream FILE_WRITE_ATTRIBUTES for PipeMode::Messages to Tokio
		if recv == Some(PipeMode::Messages) {
		set_named_pipe_handle_state(client.as_handle(), Some(PIPE_READMODE_MESSAGE), None, None)?;
		}
		*/
		Ok(Self::new_client(client))
	}
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
	/// Connects to the specified named pipe at the specified path (the `\\<hostname>\pipe\` prefix
	/// is not added automatically), waiting until a server instance is dispatched.
	#[inline]
	pub async fn connect_by_path(path: impl AsRef<Path>) -> io::Result<Self> {
		RawPipeStream::connect(path.as_ref(), Rm::MODE, Sm::MODE)
			.await
			.map(Self::new)
	}

	#[inline]
	pub(crate) async fn connect_with_prepend(
		pipename: &OsStr,
		hostname: Option<&OsStr>,
	) -> io::Result<Self> {
		RawPipeStream::connect_with_prepend(pipename, hostname, Rm::MODE, Sm::MODE)
			.await
			.map(Self::new)
	}

	/// Internal constructor used by the listener. It's a logic error, but not UB, to create the
	/// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
	pub(crate) fn new(raw: RawPipeStream) -> Self {
		Self {
			raw: MaybeArc::Inline(raw),
			flush: Mutex::new(None),
			_phantom: PhantomData,
		}
	}
}
