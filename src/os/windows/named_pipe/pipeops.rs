use crate::os::windows::{imports::*, named_pipe::stream::Instance, FileHandleOps};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    mem::ManuallyDrop,
    ptr,
};

/// The actual implementation of a named pipe server or client.
#[repr(transparent)]
pub struct PipeOps(pub(crate) FileHandleOps);
impl PipeOps {
    /// Reads a message from the pipe instance into the specified buffer, returning the size of the message written as `Ok(Ok(...))`. If the buffer is too small to fit the message, a bigger buffer is allocated and returned as `Ok(Err(...))`, with the exact size and capacity to hold the message. Errors are returned as `Err(Err(...))`.
    pub fn read_msg(&self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
        match self.try_read_msg(buf)? {
            Ok(bytes_read) => Ok(Ok(bytes_read)),
            Err(bytes_left_in_message) => {
                let mut new_buffer = vec![0; bytes_left_in_message];
                let mut _number_of_bytes_read: DWORD = 0;
                let success = unsafe {
                    ReadFile(
                        self.as_raw_handle(),
                        new_buffer.as_mut_slice().as_mut_ptr() as *mut _,
                        buf.len() as DWORD,
                        &mut _number_of_bytes_read as *mut _,
                        ptr::null_mut(),
                    ) != 0
                };
                if success {
                    Ok(Err(new_buffer))
                } else {
                    Err(io::Error::last_os_error())
                }
            }
        }
    }
    pub fn try_read_msg(&self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
        debug_assert!(
            buf.len() <= DWORD::max_value() as usize,
            "buffer is bigger than maximum buffer size for ReadFile",
        );
        let bytes_left_in_message = unsafe {
            let mut bytes_left_in_message: DWORD = 0;
            let result = PeekNamedPipe(
                self.as_raw_handle(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut bytes_left_in_message as *mut _,
            );
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
            bytes_left_in_message as usize
        };
        if buf.len() >= bytes_left_in_message {
            // We already know the exact size of the message which is why this does not matter
            let mut _number_of_bytes_read: DWORD = 0;
            let success = unsafe {
                ReadFile(
                    self.as_raw_handle(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len() as DWORD,
                    &mut _number_of_bytes_read as *mut _,
                    ptr::null_mut(),
                ) != 0
            };
            if success {
                Ok(Ok(bytes_left_in_message))
            } else {
                Err(io::Error::last_os_error())
            }
        } else {
            Ok(Err(bytes_left_in_message))
        }
    }
    /// Reads bytes from the named pipe. Mirrors `std::io::Read`.
    pub fn read_bytes(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    /// Writes data to the named pipe. There is no way to check/ensure that the message boundaries will be preserved which is why there's only one function to do this.
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    /// Blocks until the client has fully read the buffer.
    pub fn flush(&self) -> io::Result<()> {
        self.0.flush()
    }

    pub fn get_client_process_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeClientProcessId) }
    }
    pub fn get_client_session_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeClientSessionId) }
    }
    pub fn get_server_process_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeServerProcessId) }
    }
    pub fn get_server_session_id(&self) -> io::Result<u32> {
        unsafe { self.hget(GetNamedPipeServerSessionId) }
    }
    unsafe fn hget(
        &self,
        f: unsafe extern "system" fn(HANDLE, *mut u32) -> BOOL,
    ) -> io::Result<u32> {
        let mut x: u32 = 0;
        let success = unsafe { f(self.0 .0, &mut x as *mut _) != 0 };
        if success {
            Ok(x)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Retrieves whether the pipe is a server or not from the kernel directly.
    pub fn is_server(&self) -> io::Result<bool> {
        // Source: https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-getnamedpipeinfo
        const PIPE_IS_SERVER_BIT: u32 = 0x00000001;

        let flags = self.get_flags()?;
        Ok((flags & PIPE_IS_SERVER_BIT) != 0)
    }
    /// Retrieves whether the pipe has message boundaries or not from the kernel directly.
    pub fn does_pipe_have_message_boundaries(&self) -> io::Result<bool> {
        // Same source.
        const PIPE_IS_MESSAGE_BIT: u32 = 0x00000004;

        let flags = self.get_flags()?;
        Ok((flags & PIPE_IS_MESSAGE_BIT) != 0)
    }
    /* // Disabled because .get_state() is disabled.
    /// Retrieves whether the pipe is in nonblocking mode or not from the kernel directly.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        // Source: https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-getnamedpipehandlestatew
        const PIPE_IS_NONBLOCKING_BIT: u32 = 0x00000001;

        let state = self.get_state()?;
        Ok((state & PIPE_IS_NONBLOCKING_BIT) != 0)
    }
    */
    fn get_flags(&self) -> io::Result<u32> {
        let mut flags: u32 = 0;
        let success = unsafe {
            GetNamedPipeInfo(
                self.0 .0,
                &mut flags as *mut _,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ) != 0
        };
        if success {
            Ok(flags)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    // Doesn't work for server-write-only pipes, requires FILE_READ_ATTRIBUTES which I can't get
    // from CreateNamedPipe.
    /*fn get_state(&self) -> io::Result<u32> {
        let mut state: u32 = 0;
        let success = unsafe {
            GetNamedPipeHandleStateW(
                self.0 .0,
                &mut state as *mut _,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
            ) != 0
        };
        if success {
            Ok(state)
        } else {
            Err(io::Error::last_os_error())
        }
    }*/

    /// Blocks until connected. If connected, does not do anything.
    pub fn connect_server(&self) -> io::Result<()> {
        let success = unsafe { ConnectNamedPipe(self.as_raw_handle(), ptr::null_mut()) != 0 };
        if success {
            Ok(())
        } else {
            let last_error = io::Error::last_os_error();
            if last_error.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32) {
                Ok(())
            } else {
                Err(last_error)
            }
        }
    }
    /// Flushes and disconnects, obviously.
    pub fn flush_and_disconnect(&self) -> io::Result<()> {
        self.flush()?;
        self.disconnect()?;
        Ok(())
    }
    /// Disconnects without flushing. Drops all data which has been sent but not yet received on the other side, if any.
    pub fn disconnect(&self) -> io::Result<()> {
        let success = unsafe { DisconnectNamedPipe(self.as_raw_handle()) != 0 };
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    /// Called by pipe streams when dropped, used to abstract over the fact that non-async streams flush before returning the pipe to the server while async ones don't.
    pub fn server_drop_disconnect(&self) {
        let _ = self.flush_and_disconnect();
    }
}
impl Debug for PipeOps {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
#[cfg(windows)]
impl AsRawHandle for PipeOps {
    fn as_raw_handle(&self) -> HANDLE {
        self.0 .0 // I hate this nested tuple syntax.
    }
}
#[cfg(windows)]
impl IntoRawHandle for PipeOps {
    fn into_raw_handle(self) -> HANDLE {
        let self_ = ManuallyDrop::new(self);
        self_.as_raw_handle()
    }
}
#[cfg(windows)]
impl FromRawHandle for PipeOps {
    unsafe fn from_raw_handle(handle: HANDLE) -> Self {
        let fho = unsafe { FileHandleOps::from_raw_handle(handle) };
        Self(fho)
    }
}
// SAFETY: we don't expose reading/writing for immutable references of PipeInstance
unsafe impl Sync for PipeOps {}
unsafe impl Send for PipeOps {}

pub trait PipeStreamInternals {
    #[cfg(windows)]
    fn build(instance: Instance) -> Self;
}
