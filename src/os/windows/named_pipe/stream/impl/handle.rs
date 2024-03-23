use super::*;
use crate::TryClone;
use std::mem::ManuallyDrop;

impl AsHandle for RawPipeStream {
	#[inline]
	fn as_handle(&self) -> BorrowedHandle<'_> {
		self.file_handle().as_handle()
	}
}
impl TryFrom<OwnedHandle> for RawPipeStream {
	type Error = FromHandleError;

	fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
		let is_server = match is_server_from_sys(handle.as_handle()) {
			Ok(b) => b,
			Err(e) => {
				return Err(FromHandleError {
					details: FromHandleErrorKind::IsServerCheckFailed,
					cause: Some(e),
					source: Some(handle),
				})
			}
		};
		Ok(Self::new(FileHandle::from(handle), is_server))
	}
}
impl From<RawPipeStream> for OwnedHandle {
	#[inline]
	fn from(x: RawPipeStream) -> Self {
		let x = ManuallyDrop::new(x);
		let handle = unsafe { std::ptr::read(&x.handle) };
		handle.expect(LIMBO_ERR).into()
	}
}

derive_asraw!(RawPipeStream);

/// Attempts to unwrap the given stream into the raw owned handle type, returning itself back if
/// no ownership over it is available, as is the case when the stream is split.
impl<Rm: PipeModeTag, Sm: PipeModeTag> TryFrom<PipeStream<Rm, Sm>> for OwnedHandle {
	type Error = PipeStream<Rm, Sm>;
	#[inline]
	fn try_from(s: PipeStream<Rm, Sm>) -> Result<Self, Self::Error> {
		match s.raw {
			MaybeArc::Inline(x) => Ok(x.into()),
			MaybeArc::Shared(..) => Err(s),
		}
	}
}

/// Attempts to wrap the given handle into the high-level pipe stream type. If the underlying pipe
/// type is wrong or trying to figure out whether it's wrong or not caused a system call error, the
/// corresponding error condition is returned.
///
/// For more on why this can fail, see [`FromHandleError`]. Most notably, server-side send-only
/// pipes will cause "access denied" errors because they lack permissions to check whether it's a
/// server-side pipe and whether it has message boundaries.
impl<Rm: PipeModeTag, Sm: PipeModeTag> TryFrom<OwnedHandle> for PipeStream<Rm, Sm> {
	type Error = FromHandleError;
	fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
		let raw = RawPipeStream::try_from(handle)?;
		// If the wrapper type tries to receive incoming data as messages, that might break if
		// the underlying pipe has no message boundaries. Let's check for that.
		if Rm::MODE == Some(PipeMode::Messages) {
			let msg_bnd = match has_msg_boundaries_from_sys(raw.as_handle()) {
				Ok(b) => b,
				Err(e) => {
					return Err(FromHandleError {
						details: FromHandleErrorKind::MessageBoundariesCheckFailed,
						cause: Some(e),
						source: Some(raw.into()),
					})
				}
			};
			if !msg_bnd {
				return Err(FromHandleError {
					details: FromHandleErrorKind::NoMessageBoundaries,
					cause: None,
					source: Some(raw.into()),
				});
			}
		}
		Ok(Self::new(raw))
	}
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> TryClone for PipeStream<Rm, Sm> {
	fn try_clone(&self) -> io::Result<Self> {
		let handle = c_wrappers::duplicate_handle(self.as_handle())?;
		self.raw.needs_flush.on_clone();
		let mut new = RawPipeStream::new(handle.into(), self.is_server());
		new.needs_flush = NeedsFlushVal::Always.into();
		Ok(Self::new(new))
	}
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> AsHandle for PipeStream<Rm, Sm> {
	#[inline]
	fn as_handle(&self) -> BorrowedHandle<'_> {
		self.raw.as_handle()
	}
}

derive_asraw!({Rm: PipeModeTag, Sm: PipeModeTag} PipeStream<Rm, Sm>, windows);
