use super::super::local_socket_name_to_ud_socket_path;
use crate::{
    local_socket::ToLocalSocketName,
    os::unix::udsocket::tokio::{ReadHalf as ReadHalfImpl, UdStream, WriteHalf as WriteHalfImpl},
};
use std::io;

#[derive(Debug)]
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

multimacro! {
    LocalSocketStream,
    pinproj_for_unpin(UdStream),
    forward_futures_rw,
    forward_as_handle(unix),
    forward_try_handle(UdStream, unix),
}

pub struct ReadHalf(ReadHalfImpl);
multimacro! {
    ReadHalf,
    pinproj_for_unpin(ReadHalfImpl),
    forward_debug("local_socket::ReadHalf"),
    forward_futures_read,
    forward_as_handle,
}

pub struct WriteHalf(WriteHalfImpl);
multimacro! {
    WriteHalf,
    pinproj_for_unpin(WriteHalfImpl),
    forward_debug("local_socket::WriteHalf"),
    forward_futures_write,
    forward_as_handle,
}
