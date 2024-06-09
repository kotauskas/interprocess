//! Windows-specific functionality for unnamed pipes.

// TODO(2.2.0) add examples and tests

#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

use super::{security_descriptor::*, winprelude::*, FileHandle};
use crate::{
	unnamed_pipe::{Recver as PubRecver, Sender as PubSender},
	weaken_buf_init_mut, AsPtr, Sealed,
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
	/// actually uses this exact size, since it's only a hint. Set to `None` to disable the hint
	/// and rely entirely on the system's default buffer size.
	pub buffer_size_hint: Option<NonZeroUsize>,
}
impl Sealed for CreationOptions<'_> {}
impl<'sd> CreationOptions<'sd> {
	/// Starts with the default parameters for the pipe. Identical to `Default::default()`.
	pub const fn new() -> Self {
		Self {
			inheritable: false,
			security_descriptor: None,
			buffer_size_hint: None,
		}
	}

	builder_setters! {
		/// Specifies the pointer to the security descriptor for the pipe.
		///
		/// See the [associated field](#structfield.security_descriptor) for more.
		security_descriptor: Option<BorrowedSecurityDescriptor<'sd>>,
		/// Specifies whether the resulting pipe can be inherited by child processes.
		///
		/// See the [associated field](#structfield.inheritable) for more.
		inheritable: bool,
		/// Provides Windows with a hint for the buffer size for the pipe.
		///
		/// See the [associated field](#structfield.buffer_size_hint) for more.
		buffer_size_hint: Option<NonZeroUsize>,
	}

	/// Creates the pipe and returns its sending and receiving ends, or an error if one occurred.
	pub fn create(self) -> io::Result<(PubSender, PubRecver)> {
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

	/// Synonymous with [`.create()`](Self::create).
	#[inline]
	pub fn build(self) -> io::Result<(PubSender, PubRecver)> {
		self.create()
	}
}
impl Default for CreationOptions<'_> {
	fn default() -> Self {
		Self::new()
	}
}

pub(crate) fn pipe_impl() -> io::Result<(PubSender, PubRecver)> {
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
