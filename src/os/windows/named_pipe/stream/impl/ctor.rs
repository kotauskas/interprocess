use super::*;
use crate::os::windows::{named_pipe::WaitTimeout, path_conversion::*};
use widestring::U16CStr;
use windows_sys::Win32::System::Pipes::PIPE_READMODE_MESSAGE;

impl RawPipeStream {
	pub(super) fn new(handle: FileHandle, is_server: bool, nfv: NeedsFlushVal) -> Self {
		Self {
			handle: Some(handle),
			is_server,
			needs_flush: NeedsFlush::from(nfv),
			concurrency_detector: ConcurrencyDetector::new(),
		}
	}
	pub(crate) fn new_server(handle: FileHandle) -> Self {
		Self::new(handle, true, NeedsFlushVal::No)
	}
	fn new_client(handle: FileHandle) -> Self {
		Self::new(handle, false, NeedsFlushVal::No)
	}
	fn connect(
		path: &U16CStr,
		recv: Option<PipeMode>,
		send: Option<PipeMode>,
	) -> io::Result<Self> {
		let handle = loop {
			match c_wrappers::connect_without_waiting(path, recv, send, false) {
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					c_wrappers::block_for_server(path, WaitTimeout::DEFAULT)?;
					continue;
				}
				els => break els,
			}
		}?;

		if recv == Some(PipeMode::Messages) {
			c_wrappers::set_np_handle_state(
				handle.as_handle(),
				Some(PIPE_READMODE_MESSAGE),
				None,
				None,
			)?;
		}
		Ok(Self::new_client(handle))
	}
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
	/// Connects to the specified named pipe at the specified path (the `\\<hostname>\pipe\` prefix
	/// is not added automatically), blocking until a server instance is dispatched.
	#[inline]
	pub fn connect_by_path<'p>(path: impl ToWtf16<'p>) -> io::Result<Self> {
		RawPipeStream::connect(&path.to_wtf_16().map_err(to_io_error)?, Rm::MODE, Sm::MODE)
			.map(Self::new)
	}

	/// Internal constructor used by the listener. It's a logic error, but not UB, to create the
	/// thing from the wrong kind of thing, but that never ever happens, to the best of my ability.
	pub(crate) fn new(raw: RawPipeStream) -> Self {
		Self {
			raw: raw.into(),
			_phantom: PhantomData,
		}
	}
}
