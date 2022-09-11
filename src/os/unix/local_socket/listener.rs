use {
    super::{local_socket_name_to_ud_socket_path, LocalSocketStream},
    crate::{local_socket::ToLocalSocketName, os::unix::udsocket::UdStreamListener},
    std::{
        fmt::{self, Debug, Formatter},
        io,
        os::unix::io::{AsRawFd, FromRawFd, IntoRawFd},
    },
};

pub struct LocalSocketListener {
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
            .field("fd", &self.inner.as_raw_fd())
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
