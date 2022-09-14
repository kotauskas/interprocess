use {
    super::thunk_broken_pipe_to_eof,
    crate::{
        local_socket::ToLocalSocketName,
        os::windows::named_pipe::DuplexBytePipeStream as PipeStream,
    },
    std::{
        ffi::c_void,
        fmt::{self, Debug, Formatter},
        io::{self, prelude::*, IoSlice, IoSliceMut},
        os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle},
    },
};

pub struct LocalSocketStream {
    pub(super) inner: PipeStream,
}
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeStream::connect(name.inner())?;
        Ok(Self { inner })
    }
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.inner.is_server() {
            true => self.inner.client_process_id(),
            false => self.inner.server_process_id(),
        }
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}

/// Thunks broken pipe errors into EOFs because broken pipe to the writer is what EOF is to the
/// reader, but Windows shoehorns both into the former.
impl Read for LocalSocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        thunk_broken_pipe_to_eof(self.inner.read(buf))
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        thunk_broken_pipe_to_eof(self.inner.read_vectored(bufs))
    }
}
impl Write for LocalSocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for LocalSocketStream {
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
impl IntoRawHandle for LocalSocketStream {
    fn into_raw_handle(self) -> *mut c_void {
        self.inner.into_raw_handle()
    }
}
impl FromRawHandle for LocalSocketStream {
    unsafe fn from_raw_handle(handle: *mut c_void) -> Self {
        let inner = unsafe {
            // SAFETY: guaranteed via safety contract
            PipeStream::from_raw_handle(handle)
        };
        Self { inner }
    }
}
