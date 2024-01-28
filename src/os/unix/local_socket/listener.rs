use super::{name_to_addr, LocalSocketStream};
use crate::local_socket::LocalSocketName;
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::unix::{io::AsRawFd, net::UnixListener},
};

pub struct LocalSocketListener(UnixListener);
impl LocalSocketListener {
    pub fn bind(name: LocalSocketName<'_>) -> io::Result<Self> {
        UnixListener::bind_addr(&name_to_addr(name)?).map(Self)
    }
    #[inline]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        // TODO make use of the second return value
        self.0.accept().map(|(s, _)| LocalSocketStream(s))
    }
    #[inline]
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
