use super::{name_to_addr, LocalSocketStream};
use crate::local_socket::ToLocalSocketName;
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::unix::{io::AsRawFd, net::UnixListener},
};

pub struct LocalSocketListener(UnixListener);
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let addr = name_to_addr(name.to_local_socket_name()?)?;
        let inner = UnixListener::bind_addr(&addr)?;
        Ok(Self(inner))
    }
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.0.accept()?.0; // TODO make use of the second return value
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
multimacro! { LocalSocketListener,
    forward_handle(unix),
    derive_trivial_conv(UnixListener),
}
