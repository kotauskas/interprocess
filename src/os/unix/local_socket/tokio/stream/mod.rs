mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;

use super::super::local_socket_name_to_ud_socket_path;
use crate::{local_socket::ToLocalSocketName, os::unix::udsocket::tokio::UdStream};
use std::{io, os::unix::io::AsRawFd, pin::Pin};

pub struct LocalSocketStream(pub(super) UdStream);
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        UdStream::connect(path).await.map(Self::from)
    }
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
}
impl From<UdStream> for LocalSocketStream {
    #[inline]
    fn from(inner: UdStream) -> Self {
        Self(inner)
    }
}
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("fd", &self.0.as_raw_fd())
            .finish()
    }
}

multimacro! {
    LocalSocketStream,
    pinproj_for_unpin(UdStream),
    forward_futures_rw,
    forward_as_handle(unix),
    forward_try_handle(UdStream, unix),
}
