use super::*;
use std::mem::ManuallyDrop;

impl AsHandle for InnerTokio {
	#[inline]
	fn as_handle(&self) -> BorrowedHandle<'_> {
		same_clsrv!(x in self => x.as_handle())
	}
}
derive_asraw!(InnerTokio);

impl AsHandle for RawPipeStream {
	#[inline]
	fn as_handle(&self) -> BorrowedHandle<'_> {
		self.inner().as_handle()
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

		let rh = handle.as_raw_handle();
		let handle = ManuallyDrop::new(handle);

		let tkresult = unsafe {
			match is_server {
				true => TokioNPServer::from_raw_handle(rh).map(InnerTokio::Server),
				false => TokioNPClient::from_raw_handle(rh).map(InnerTokio::Client),
			}
		};
		match tkresult {
			Ok(s) => Ok(Self::new(s)),
			Err(e) => Err(FromHandleError {
				details: FromHandleErrorKind::TokioError,
				cause: Some(e),
				source: Some(ManuallyDrop::into_inner(handle)),
			}),
		}
	}
}

// Tokio does not implement TryInto<OwnedHandle>
derive_asraw!(RawPipeStream);

impl<Rm: PipeModeTag, Sm: PipeModeTag> AsHandle for PipeStream<Rm, Sm> {
	fn as_handle(&self) -> BorrowedHandle<'_> {
		self.raw.as_handle()
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
		// If the wrapper type tries to receive incoming data as messages, that might break if
		// the underlying pipe has no message boundaries. Let's check for that.
		if Rm::MODE == Some(PipeMode::Messages) {
			let msg_bnd = match has_msg_boundaries_from_sys(handle.as_handle()) {
				Ok(b) => b,
				Err(e) => {
					return Err(FromHandleError {
						details: FromHandleErrorKind::MessageBoundariesCheckFailed,
						cause: Some(e),
						source: Some(handle),
					})
				}
			};
			if !msg_bnd {
				return Err(FromHandleError {
					details: FromHandleErrorKind::NoMessageBoundaries,
					cause: None,
					source: Some(handle),
				});
			}
		}
		let raw = RawPipeStream::try_from(handle)?;
		Ok(Self::new(raw))
	}
}

derive_asraw!({Rm: PipeModeTag, Sm: PipeModeTag} PipeStream<Rm, Sm>, windows);
