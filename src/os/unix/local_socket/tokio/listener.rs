use super::LocalSocketStream;
use crate::{
    local_socket::LocalSocketName,
    os::unix::local_socket::{
        listener::LocalSocketListener as SyncLocalSocketListener, ReclaimGuard,
    },
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::unix::prelude::*,
};
use tokio::net::UnixListener;

pub struct LocalSocketListener {
    listener: UnixListener,
    reclaim: ReclaimGuard,
}
impl LocalSocketListener {
    pub fn bind(name: LocalSocketName<'_>, keep_name: bool) -> io::Result<Self> {
        Self::try_from(SyncLocalSocketListener::bind(name, keep_name)?)
    }
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.listener.accept().await?.0;
        Ok(LocalSocketStream(inner))
    }

    pub fn do_not_reclaim_name_on_drop(&mut self) {
        self.reclaim.forget();
    }
}

impl TryFrom<SyncLocalSocketListener> for LocalSocketListener {
    type Error = io::Error;
    fn try_from(mut sync: SyncLocalSocketListener) -> io::Result<Self> {
        sync.set_nonblocking(true)?;
        let reclaim = sync.reclaim.take();
        Ok(Self {
            listener: UnixListener::from_std(sync.into())?,
            reclaim,
        })
    }
}

impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketListener")
            .field("fd", &self.listener.as_raw_fd())
            .field("reclaim", &self.reclaim)
            .finish()
    }
}
impl AsFd for LocalSocketListener {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }
}
impl TryFrom<LocalSocketListener> for OwnedFd {
    type Error = io::Error;
    fn try_from(mut slf: LocalSocketListener) -> io::Result<Self> {
        slf.listener.into_std().map(|s| {
            slf.reclaim.forget();
            s.into()
        })
    }
}
impl TryFrom<OwnedFd> for LocalSocketListener {
    type Error = io::Error;
    fn try_from(fd: OwnedFd) -> io::Result<Self> {
        Self::try_from(SyncLocalSocketListener::from(fd))
    }
}
