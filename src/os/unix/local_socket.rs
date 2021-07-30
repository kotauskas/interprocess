use super::udsocket::{UdSocketPath, UdStream, UdStreamListener};
use crate::local_socket::{LocalSocketName, NameTypeSupport, ToLocalSocketName};
use std::{
    borrow::Cow,
    ffi::{CStr, CString, OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io::{self, prelude::*, IoSlice, IoSliceMut},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawFd, FromRawFd, IntoRawFd},
    },
};

pub(crate) struct LocalSocketListener {
    inner: UdStreamListener,
}
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStreamListener::bind(path)?;
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
        f.debug_struct("LocalSocketListener")
            .field("file_descriptor", &self.inner.as_raw_fd())
            .finish()
    }
}
impl AsRawFd for LocalSocketListener {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}
impl IntoRawFd for LocalSocketListener {
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}
impl FromRawFd for LocalSocketListener {
    unsafe fn from_raw_fd(fd: i32) -> Self {
        Self {
            inner: unsafe { UdStreamListener::from_raw_fd(fd) },
        }
    }
}

pub(crate) struct LocalSocketStream {
    inner: UdStream,
}
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStream::connect(path)?;
        Ok(Self { inner })
    }
    pub fn peer_pid(&self) -> io::Result<u32> {
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            self.inner
                .get_peer_credentials()
                .map(|ucred| ucred.pid as u32)
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            Err(io::Error::new(io::ErrorKind::Other, "not supported"))
        }
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}
impl Read for LocalSocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
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
            .field("file_descriptor", &self.inner.as_raw_fd())
            .finish()
    }
}
impl AsRawFd for LocalSocketStream {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}
impl IntoRawFd for LocalSocketStream {
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}
impl FromRawFd for LocalSocketStream {
    unsafe fn from_raw_fd(fd: i32) -> Self {
        Self {
            inner: unsafe { UdStream::from_raw_fd(fd) },
        }
    }
}

fn local_socket_name_to_ud_socket_path(name: LocalSocketName<'_>) -> io::Result<UdSocketPath<'_>> {
    fn cow_osstr_to_cstr(osstr: Cow<'_, OsStr>) -> io::Result<Cow<'_, CStr>> {
        match osstr {
            Cow::Borrowed(val) => {
                if val.as_bytes().last() == Some(&0) {
                    Ok(Cow::Borrowed(
                        CStr::from_bytes_with_nul(val.as_bytes())
                            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
                    ))
                } else {
                    let owned = val.to_os_string();
                    Ok(Cow::Owned(CString::new(owned.into_vec())?))
                }
            }
            Cow::Owned(val) => Ok(Cow::Owned(CString::new(val.into_vec())?)),
        }
    }
    #[cfg(target_os = "linux")]
    if name.is_namespaced() {
        return Ok(UdSocketPath::Namespaced(cow_osstr_to_cstr(
            name.into_inner_cow(),
        )?));
    }
    Ok(UdSocketPath::File(cow_osstr_to_cstr(
        name.into_inner_cow(),
    )?))
}

pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(target_os = "linux")]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(target_os = "linux"))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyPaths;

const AT_SIGN: u8 = 0x40;

pub fn to_local_socket_name_osstr(mut val: &OsStr) -> LocalSocketName<'_> {
    let mut namespaced = false;
    if let Some(AT_SIGN) = val.as_bytes().get(0).copied() {
        if val.len() >= 2 {
            val = OsStr::from_bytes(&val.as_bytes()[1..]);
        } else {
            val = OsStr::from_bytes(&[]);
        }
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Borrowed(val), namespaced)
}
pub fn to_local_socket_name_osstring(mut val: OsString) -> LocalSocketName<'static> {
    let mut namespaced = false;
    if let Some(AT_SIGN) = val.as_bytes().get(0).copied() {
        let new_val = {
            let mut vec = val.into_vec();
            vec.remove(0);
            OsString::from_vec(vec)
        };
        val = new_val;
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Owned(val), namespaced)
}
