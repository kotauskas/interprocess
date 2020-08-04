//! Platform-specific functionality for unnamed pipes.
//!
//! Currently, this consists of only the [`UnnamedPipeCreationOptions`] builder, but more might be added.
//!
//! [`UnnamedPipeCreationOptions`]: struct.UnnamedPipeCreationOptions.html " "

// TODO add examples

use std::{
    os::windows::io::{FromRawHandle, AsRawHandle, IntoRawHandle},
    mem::{self, zeroed, size_of},
    io::{self, Read, Write},
    num::NonZeroUsize,
    fmt::{self, Formatter, Debug},
};
use winapi::{
    shared::{
        ntdef::{HANDLE, NULL},
        minwindef::LPVOID,
    },
    um::{
        minwinbase::SECURITY_ATTRIBUTES,
        handleapi::INVALID_HANDLE_VALUE,
        namedpipeapi::CreatePipe,
    },
};
use crate::{
    unnamed_pipe::{
        UnnamedPipeReader as PubReader,
        UnnamedPipeWriter as PubWriter,
    },
};
use super::FileHandleOps;

/// Builder used to create unnamed pipes while supplying additional options.
///
/// You can use this instead of the simple [`pipe` function] to supply additional Windows-specific parameters to a pipe.
///
/// [`pipe` function]: ../../../unnamed_pipe/fn.pipe.html " "
#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub struct UnnamedPipeCreationOptions {
    /// Specifies whether the resulting pipe can be inherited by child processes.
    ///
    /// The default value is `true` and you probably shouldn't modify this, unless you want all child processes to explicitly be unable to use the pipe even if they attempt to use various fishy methods to find the handle in the parent process.
    pub inheritable: bool,
    /// A pointer to the [security descriptor] for the pipe. Leave this at the default `NULL` unless you want something specific.
    ///
    /// [security descriptor]: https://docs.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-security_descriptor " "
    pub security_descriptor: LPVOID,
    /// A hint on the buffer size for the pipe. There is no way to ensure or check that the system actually uses this exact size, since it's only a hint. Set to `None` to disable the hint and rely entirely on the system's default buffer size.
    pub buffer_size_hint: Option<NonZeroUsize>,
}
impl UnnamedPipeCreationOptions {
    /// Starts with the default parameters for the pipe. Identical to `Default::default()`.
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            inheritable: true,
            security_descriptor: NULL,
            buffer_size_hint: None,
        }
    }
    /// Specifies whether the resulting pipe can be inherited by child processes.
    ///
    /// See the [associated field] for more.
    ///
    /// [associated field]: #structfield.inheritable " "
    #[inline]
    #[must_use = "this is not an in-place operation"]
    pub fn inheritable(mut self, inheritable: bool) -> Self {
        self.inheritable = inheritable;
        self
    }
    /// Specifies the pointer to the security descriptor for the pipe.
    ///
    /// See the [associated field] for more.
    ///
    /// [associated field]: #structfield.security_descriptor " "
    #[must_use = "this is not an in-place operation"]
    pub fn security_descriptor(mut self, security_descriptor: LPVOID) -> Self {
        self.security_descriptor = security_descriptor;
        self
    }
    /// Specifies the hint on the buffer size for the pipe.
    ///
    /// See the [associated field] for more.
    ///
    /// [associated field]: #structfield.buffer_size_hint " "
    #[must_use = "this is not an in-place operation"]
    pub fn buffer_size_hint(mut self, buffer_size_hint: Option<NonZeroUsize>) -> Self {
        self.buffer_size_hint = buffer_size_hint;
        self
    }

    /// Extracts the [`SECURITY_ATTRIBUTES`] from the builder. Primarily an implementation detail, but has other uses.
    ///
    /// [`SECURITY_ATTRIBUTES`]: https://docs.microsoft.com/en-us/previous-versions/windows/desktop/legacy/aa379560(v=vs.85)
    #[inline]
    pub fn extract_security_attributes(self) -> SECURITY_ATTRIBUTES {
        // Safe because WinAPI parameter structs are typically rejected if a required field is zero
        let mut security_attributes = unsafe {zeroed::<SECURITY_ATTRIBUTES>()};
        security_attributes.nLength = size_of::<SECURITY_ATTRIBUTES>() as u32;
        security_attributes.lpSecurityDescriptor = self.security_descriptor;
        security_attributes.bInheritHandle = self.inheritable as i32;
        security_attributes
    }

    /// Creates the pipe and returns its writing and reading ends, or the error if one occurred.
    ///
    /// # Safety
    /// The [`security_descriptor`] field is passed directly to Win32 which is then dereferenced there, resulting in undefined behavior if it was an invalid non-null pointer. For the default configuration, this should never be a concern.
    ///
    /// [`security_descriptor`]: #field.security_descriptor " "
    #[inline]
    pub unsafe fn build(self) -> io::Result<(PubWriter, PubReader)> {
        let hint_raw = match self.buffer_size_hint {
            Some(num) => num.get(),
            None => 0,
        } as u32;
        let [mut writer, mut reader] = [INVALID_HANDLE_VALUE; 2];
        let success = CreatePipe(
            &mut reader as *mut _,
            &mut writer as *mut _,
            &mut self.extract_security_attributes() as *mut _,
            hint_raw,
        ) != 0;
        if success {
            // SAFETY: we just created those handles which means that we own them
            Ok((
                PubWriter {
                    inner: UnnamedPipeWriter::from_raw_handle(writer),
                },
                PubReader {
                    inner: UnnamedPipeReader::from_raw_handle(reader),
                },
            ))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
impl Default for UnnamedPipeCreationOptions {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}
// FIXME the Sync trait is also probably fine since those are just system calls, but I'm not sure
// yet.
unsafe impl Send for UnnamedPipeCreationOptions {}

#[inline(always)]
pub(crate) fn pipe() -> io::Result<(PubWriter, PubReader)> {
    unsafe {UnnamedPipeCreationOptions::default().build()}
}

pub(crate) struct UnnamedPipeReader (FileHandleOps);
impl Read for UnnamedPipeReader {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}
impl AsRawHandle for UnnamedPipeReader {
    #[inline(always)]
    fn as_raw_handle(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}
impl IntoRawHandle for UnnamedPipeReader {
    #[inline]
    fn into_raw_handle(self) -> HANDLE {
        let handle = self.as_raw_handle();
        mem::forget(self);
        handle
    }
}
impl FromRawHandle for UnnamedPipeReader {
    #[inline(always)]
    unsafe fn from_raw_handle(handle: HANDLE) -> Self {
        Self(FileHandleOps::from_raw_handle(handle))
    }
}
impl Debug for UnnamedPipeReader {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("UnnamedPipeReader")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}

pub(crate) struct UnnamedPipeWriter (FileHandleOps);
impl Write for UnnamedPipeWriter {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl AsRawHandle for UnnamedPipeWriter {
    #[inline(always)]
    fn as_raw_handle(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}
impl IntoRawHandle for UnnamedPipeWriter {
    #[inline]
    fn into_raw_handle(self) -> HANDLE {
        let handle = self.as_raw_handle();
        mem::forget(self);
        handle
    }
}
impl FromRawHandle for UnnamedPipeWriter {
    #[inline(always)]
    unsafe fn from_raw_handle(handle: HANDLE) -> Self {
        Self(FileHandleOps(handle))
    }
}
impl Debug for UnnamedPipeWriter {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("UnnamedPipeWriter")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}