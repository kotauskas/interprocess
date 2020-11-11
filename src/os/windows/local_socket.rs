use std::{
    io::{self, IoSlice, IoSliceMut, prelude::*},
    fmt::{self, Formatter, Debug},
    ffi::{OsStr, OsString, c_void},
    //path::{Path, PathBuf},
    borrow::Cow,
    os::windows::io::{AsRawHandle, IntoRawHandle, FromRawHandle},
};
use crate::local_socket::{
    NameTypeSupport,
    LocalSocketName,
    ToLocalSocketName,
};
use super::named_pipe::{
    PipeListener as GenericPipeListener,
    DuplexBytePipeStream as PipeStream,
    PipeListenerOptions,
    PipeMode,
};

type PipeListener = GenericPipeListener<PipeStream>;

pub struct LocalSocketListener {
    inner: PipeListener,
}
impl LocalSocketListener {
    #[inline]
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create()?;
        Ok(Self {inner})
    }
    #[inline(always)]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept()?;
        Ok(LocalSocketStream {inner})
    }
}
impl Debug for LocalSocketListener {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
        Ok(Self {inner})
    }
}
impl Read for LocalSocketStream {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
    #[inline(always)]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }
}
impl Write for LocalSocketStream {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    #[inline(always)]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl Debug for LocalSocketStream {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("handle", &self.as_raw_handle())
            .finish()
    }
}
impl AsRawHandle for LocalSocketStream {
    #[inline(always)]
    fn as_raw_handle(&self) -> *mut c_void {
        self.inner.as_raw_handle()
    }
}
impl IntoRawHandle for LocalSocketStream {
    #[inline(always)]
    fn into_raw_handle(self) -> *mut c_void {
        self.inner.into_raw_handle()
    }
}
impl FromRawHandle for LocalSocketStream {
    #[inline(always)]
    unsafe fn from_raw_handle(handle: *mut c_void) -> Self {
        Self {inner: PipeStream::from_raw_handle(handle)}
    }
}

pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyNamespaced;

#[inline(always)]
pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
#[inline(always)]
pub fn to_local_socket_name_osstr(osstr: &OsStr) -> LocalSocketName<'_> {
    LocalSocketName::from_raw_parts(Cow::Borrowed(osstr), true)
}
#[inline(always)]
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