use super::LocalSocketStream;
use crate::{
    local_socket::LocalSocketName,
    os::unix::local_socket::{listener as synclistener, name_to_addr},
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::{
        fd::OwnedFd,
        unix::{io::AsRawFd, net::UnixListener as SyncUnixListener},
    },
};
use tokio::net::UnixListener;

pub struct LocalSocketListener(UnixListener);
impl LocalSocketListener {
    pub fn bind(name: LocalSocketName<'_>) -> io::Result<Self> {
        let sync = SyncUnixListener::bind_addr(&name_to_addr(name)?)?;
        sync.set_nonblocking(true)?;
        Ok(UnixListener::from_std(sync)?.into())
    }
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.0.accept().await?.0;
        Ok(LocalSocketStream(inner))
    }
}
impl From<UnixListener> for LocalSocketListener {
    #[inline]
    fn from(inner: UnixListener) -> Self {
        Self(inner)
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketListener").field("fd", &self.0.as_raw_fd()).finish()
    }
}
multimacro! {
    LocalSocketListener,
    forward_as_handle(unix),
}
impl TryFrom<LocalSocketListener> for OwnedFd {
    type Error = io::Error;
    fn try_from(slf: LocalSocketListener) -> Result<Self, Self::Error> {
        Ok(slf.0.into_std()?.into())
    }
}
impl TryFrom<OwnedFd> for LocalSocketListener {
    type Error = io::Error;
    fn try_from(fd: OwnedFd) -> Result<Self, Self::Error> {
        Ok(UnixListener::from_std(synclistener::LocalSocketListener::from(fd).into())?.into())
    }
}
