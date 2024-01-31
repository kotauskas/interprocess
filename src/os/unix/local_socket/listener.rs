use super::{name_to_addr, LocalSocketStream, ReclaimGuard};
use crate::local_socket::LocalSocketName;
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::{io::AsRawFd, net::UnixListener},
    },
};

pub struct LocalSocketListener {
    pub(super) listener: UnixListener,
    pub(super) reclaim: ReclaimGuard,
}
impl LocalSocketListener {
    pub fn bind(name: LocalSocketName<'_>, keep_name: bool) -> io::Result<Self> {
        Ok(Self {
            listener: UnixListener::bind_addr(&name_to_addr(name.borrow())?)
                .map_err(Self::decode_listen_error)?,
            reclaim: keep_name
                .then_some(name.into_owned())
                .map(ReclaimGuard::new)
                .unwrap_or_default(),
        })
    }

    fn decode_listen_error(error: io::Error) -> io::Error {
        io::Error::from(match error.kind() {
            io::ErrorKind::AlreadyExists => io::ErrorKind::AddrInUse,
            _ => return error,
        })
    }

    #[inline]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        // TODO make use of the second return value
        self.listener.accept().map(|(s, _)| LocalSocketStream(s))
    }
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.listener.set_nonblocking(nonblocking)
    }
    pub fn do_not_reclaim_name_on_drop(&mut self) {
        self.reclaim.forget();
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

impl From<LocalSocketListener> for UnixListener {
    fn from(mut l: LocalSocketListener) -> Self {
        l.reclaim.forget();
        l.listener
    }
}

impl AsFd for LocalSocketListener {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }
}
impl From<LocalSocketListener> for OwnedFd {
    fn from(l: LocalSocketListener) -> Self {
        UnixListener::from(l).into()
    }
}
impl From<OwnedFd> for LocalSocketListener {
    fn from(fd: OwnedFd) -> Self {
        LocalSocketListener {
            listener: fd.into(),
            reclaim: ReclaimGuard::default(),
        }
    }
}
