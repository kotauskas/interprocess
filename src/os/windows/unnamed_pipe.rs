//! Platform-specific functionality for unnamed pipes.
//!
//! Currently, this consists of only the [`UnnamedPipeCreationOptions`] builder, but more might be
//! added.

// TODO add examples

use super::{winprelude::*, FileHandle, SecurityDescriptor};
use crate::{
	unnamed_pipe::{UnnamedPipeRecver as PubRecver, UnnamedPipeSender as PubSender},
	weaken_buf_init_mut,
};
use std::{
	fmt::{self, Debug, Formatter},
	io::{self, Read, Write},
	num::NonZeroUsize,
};
use windows_sys::Win32::{Security::SECURITY_ATTRIBUTES, System::Pipes::CreatePipe};

/// Builder used to create unnamed pipes while supplying additional options.
///
/// You can use this instead of the simple [`pipe` function](crate::unnamed_pipe::pipe) to supply
/// additional Windows-specific parameters to a pipe.
#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub struct UnnamedPipeCreationOptions<'a> {
	/// A security descriptor for the pipe.
	pub security_descriptor: Option<&'a SecurityDescriptor>,
	/// Specifies whether the resulting pipe can be inherited by child processes.
	///
	/// The default value is `true`.
	pub inheritable: bool,
	/// A hint on the buffer size for the pipe. There is no way to ensure or check that the system
	/// actually uses this exact size, since it's only a hint. Set to `None` to disable the hint and
	/// rely entirely on the system's default buffer size.
	pub buffer_size_hint: Option<NonZeroUsize>,
}
impl<'a> UnnamedPipeCreationOptions<'a> {
	/// Starts with the default parameters for the pipe. Identical to `Default::default()`.
	pub const fn new() -> Self {
		Self {
			inheritable: false,
			security_descriptor: None,
			buffer_size_hint: None,
		}
	}
	/// Specifies the pointer to the security descriptor for the pipe.
	///
	/// See the [associated field](#structfield.security_descriptor) for more.
	#[must_use = "this is not an in-place operation"]
	#[inline]
	pub fn security_descriptor(
		mut self,
		security_descriptor: Option<&'a SecurityDescriptor>,
	) -> Self {
		self.security_descriptor = security_descriptor;
		self
	}
	/// Specifies whether the resulting pipe can be inherited by child processes.
	///
	/// See the [associated field](#structfield.inheritable) for more.
	#[must_use = "this is not an in-place operation"]
	#[inline]
	pub fn inheritable(mut self, inheritable: bool) -> Self {
		self.inheritable = inheritable;
		self
	}
	/// Specifies the hint on the buffer size for the pipe.
	///
	/// See the [associated field](#structfield.buffer_size_hint) for more.
	#[must_use = "this is not an in-place operation"]
	#[inline]
	pub fn buffer_size_hint(mut self, buffer_size_hint: Option<NonZeroUsize>) -> Self {
		self.buffer_size_hint = buffer_size_hint;
		self
	}

	/// Creates the pipe and returns its sending and receiving ends, or the error if one
	/// occurred.
	pub fn build(self) -> io::Result<(PubSender, PubRecver)> {
		let hint_raw = match self.buffer_size_hint {
			Some(num) => num.get(),
			None => 0,
		} as u32;

		let sd = SecurityDescriptor::create_security_attributes(
			self.security_descriptor,
			self.inheritable,
		);

		let [mut w, mut r] = [INVALID_HANDLE_VALUE; 2];
		let success = unsafe {
			CreatePipe(
				&mut r,
				&mut w,
				(&sd as *const SECURITY_ATTRIBUTES).cast_mut().cast(),
				hint_raw,
			)
		} != 0;
		if success {
			let (w, r) = unsafe {
				// SAFETY: we just created those handles which means that we own them
				let w = OwnedHandle::from_raw_handle(w as RawHandle);
				let r = OwnedHandle::from_raw_handle(r as RawHandle);
				(w, r)
			};
			let w = PubSender(UnnamedPipeSender(FileHandle::from(w)));
			let r = PubRecver(UnnamedPipeRecver(FileHandle::from(r)));
			Ok((w, r))
		} else {
			Err(io::Error::last_os_error())
		}
	}
}
impl Default for UnnamedPipeCreationOptions<'_> {
	fn default() -> Self {
		Self::new()
	}
}

pub(crate) fn pipe() -> io::Result<(PubSender, PubRecver)> {
	UnnamedPipeCreationOptions::default().build()
}

pub(crate) struct UnnamedPipeRecver(FileHandle);
impl Read for UnnamedPipeRecver {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.0.read(weaken_buf_init_mut(buf))
	}
}
impl Debug for UnnamedPipeRecver {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("UnnamedPipeRecver")
			.field(&self.0.as_raw_handle())
			.finish()
	}
}
multimacro! {
	UnnamedPipeRecver,
	forward_handle,
	forward_try_clone,
}

pub(crate) struct UnnamedPipeSender(FileHandle);
impl Write for UnnamedPipeSender {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.0.write(buf)
	}
	fn flush(&mut self) -> io::Result<()> {
		self.0.flush()
	}
}
impl Debug for UnnamedPipeSender {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("UnnamedPipeSender")
			.field(&self.0.as_raw_handle())
			.finish()
	}
}
multimacro! {
	UnnamedPipeSender,
	forward_handle,
	forward_try_clone,
}
