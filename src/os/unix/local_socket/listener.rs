use {
    super::{local_socket_name_to_ud_socket_path, LocalSocketStream},
    crate::{local_socket::ToLocalSocketName, os::unix::udsocket::UdStreamListener},
    std::{
        fmt::{self, Debug, Formatter},
        io,
        os::unix::io::AsRawFd,
    },
};

pub struct LocalSocketListener(UdStreamListener);
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStreamListener::bind(path)?;
        Ok(Self(inner))
    }
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.0.accept()?;
        Ok(LocalSocketStream(inner))
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketListener")
            .field("fd", &self.0.as_raw_fd())
            .finish()
    }
}
forward_handle!(LocalSocketListener, unix);
