use super::named_pipe::{
    DuplexBytePipeStream as PipeStream, PipeListener as GenericPipeListener, PipeListenerOptions,
    PipeMode,
};
use crate::local_socket::{LocalSocketName, NameTypeSupport, ToLocalSocketName};
use std::{
    borrow::Cow,
    ffi::{c_void, OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io::{self, prelude::*, IoSlice, IoSliceMut},
    os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle},
};

type PipeListener = GenericPipeListener<PipeStream>;

pub struct LocalSocketListener {
    inner: PipeListener,
}
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create()?;
        Ok(Self { inner })
    }
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept()?;
        Ok(LocalSocketStream { inner })
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("LocalSocketListener")
    }
}

pub struct LocalSocketStream {
    inner: PipeStream,
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

fn thunk_broken_pipe_to_eof(r: io::Result<usize>) -> io::Result<usize> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(0),
        els => els,
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

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNamespaced;

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
pub fn to_local_socket_name_osstr(osstr: &OsStr) -> LocalSocketName<'_> {
    LocalSocketName::from_raw_parts(Cow::Borrowed(osstr), true)
}
pub fn to_local_socket_name_osstring(osstring: OsString) -> LocalSocketName<'static> {
    LocalSocketName::from_raw_parts(Cow::Owned(osstring), true)
}

/*
/// Helper function to check whether a series of UTF-16 bytes starts with `\\.\pipe\`.
fn has_pipefs_prefix(
    val: impl IntoIterator<Item = u16>,
) -> bool {
    let pipefs_prefix: [u16; 9] = [
        // The string \\.\pipe\ in UTF-16
        0x005c, 0x005c, 0x002e, 0x005c, 0x0070, 0x0069, 0x0070, 0x0065, 0x005c,
    ];
    pipefs_prefix.iter().copied().eq(val)

}*/

// TODO add Path/PathBuf special-case for \\.\pipe\*
