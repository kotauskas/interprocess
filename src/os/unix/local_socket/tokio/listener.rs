use {
    super::{super::local_socket_name_to_ud_socket_path, LocalSocketStream},
    crate::{local_socket::ToLocalSocketName, os::unix::udsocket::tokio::UdStreamListener},
    std::{
        fmt::{self, Debug, Formatter},
        io,
        os::unix::io::AsRawFd,
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
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept().await?;
        Ok(LocalSocketStream { inner })
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
