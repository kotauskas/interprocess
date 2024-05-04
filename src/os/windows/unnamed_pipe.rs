//! Platform-specific functionality for unnamed pipes.
//!
//! Currently, this consists of only the [`CreationOptions`] builder, but more might be
//! added.

// TODO(2.0.2) add examples and tests

use super::{security_descriptor::*, winprelude::*, FileHandle};
use crate::{
	unnamed_pipe::{Recver as PubRecver, Sender as PubSender},
	weaken_buf_init_mut, AsPtr,
};
use std::{
	fmt::{self, Debug, Formatter},
	io::{self, Read, Write},
	num::NonZeroUsize,
};
use windows_sys::Win32::System::Pipes::CreatePipe;

/// Builder used to create unnamed pipes while supplying additional options.
///
/// You can use this instead of the simple [`pipe` function](crate::unnamed_pipe::pipe) to supply
/// additional Windows-specific parameters to a pipe.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct CreationOptions<'sd> {
	/// Security descriptor for the pipe.
	pub security_descriptor: Option<BorrowedSecurityDescriptor<'sd>>,
	/// Specifies whether the resulting pipe can be inherited by child processes.
	///
	/// The default value is `true`.
	pub inheritable: bool,
	/// Hint on the buffer size for the pipe. There is no way to ensure or check that the system
	/// actually uses this exact size, since it's only a hint. Set to `None` to disable the hint and
	/// rely entirely on the system's default buffer size.
	pub buffer_size_hint: Option<NonZeroUsize>,
}
impl<'sd> CreationOptions<'sd> {
	/// Starts with the default parameters for the pipe. Identical to `Default::default()`.
	pub const fn new() -> Self {
		Self {
			inheritable: false,
			security_descriptor: None,
			buffer_size_hint: None,
		}
	}
	// TODO(2.0.2) use macro
	/// Specifies the pointer to the security descriptor for the pipe.
	///
	/// See the [associated field](#structfield.security_descriptor) for more.
	#[must_use = builder_must_use!()]
	#[inline]
	pub fn security_descriptor(
		mut self,
		security_descriptor: Option<BorrowedSecurityDescriptor<'sd>>,
	) -> Self {
		self.security_descriptor = security_descriptor;
		self
	}
	/// Specifies whether the resulting pipe can be inherited by child processes.
	///
	/// See the [associated field](#structfield.inheritable) for more.
	#[must_use = builder_must_use!()]
	#[inline]
	pub fn inheritable(mut self, inheritable: bool) -> Self {
		self.inheritable = inheritable;
		self
	}
	/// Specifies the hint on the buffer size for the pipe.
	///
	/// See the [associated field](#structfield.buffer_size_hint) for more.
	#[must_use = builder_must_use!()]
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
		}
		.try_into()
		.unwrap();

		let sd = create_security_attributes(self.security_descriptor, self.inheritable);

		let [mut w, mut r] = [INVALID_HANDLE_VALUE; 2];
		let success =
			unsafe { CreatePipe(&mut r, &mut w, sd.as_ptr().cast_mut().cast(), hint_raw) } != 0;
		if success {
			let (w, r) = unsafe {
				// SAFETY: we just created those handles which means that we own them
				let w = OwnedHandle::from_raw_handle(w.to_std());
				let r = OwnedHandle::from_raw_handle(r.to_std());
				(w, r)
			};
			let w = PubSender(Sender(FileHandle::from(w)));
			let r = PubRecver(Recver(FileHandle::from(r)));
			Ok((w, r))
		} else {
			Err(io::Error::last_os_error())
		}
	}
}
impl Default for CreationOptions<'_> {
	fn default() -> Self {
		Self::new()
	}
}

pub(crate) fn pipe() -> io::Result<(PubSender, PubRecver)> {
	CreationOptions::default().build()
}

pub(crate) struct Recver(FileHandle);
impl Read for Recver {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.0.read(weaken_buf_init_mut(buf))
	}
}
impl Debug for Recver {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Recver")
			.field(&self.0.as_raw_handle())
			.finish()
	}
}
multimacro! {
	Recver,
	forward_handle,
	forward_try_clone,
}

pub(crate) struct Sender(FileHandle);
impl Write for Sender {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.0.write(buf)
	}
	fn flush(&mut self) -> io::Result<()> {
		self.0.flush()
	}
}
impl Debug for Sender {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Sender")
			.field(&self.0.as_raw_handle())
			.finish()
	}
}
multimacro! {
	Sender,
	forward_handle,
	forward_try_clone,
}
