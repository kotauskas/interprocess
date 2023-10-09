use super::local_socket_name_to_ud_socket_path;
use crate::{
    local_socket::ToLocalSocketName,
    os::unix::udsocket::{UdSocket, UdStream},
};
use std::{io, sync::Arc};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) UdStream);
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStream::connect(path)?;
        Ok(Self(inner))
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let arc = Arc::new(self);
        (ReadHalf(Arc::clone(&arc)), WriteHalf(arc))
    }
}
multimacro! {
    LocalSocketStream,
    forward_rbv(UdStream, &),
    forward_sync_ref_rw,
    forward_handle(unix),
    derive_sync_mut_rw,
}

#[derive(Debug)]
pub struct ReadHalf(pub(super) Arc<LocalSocketStream>);
multimacro! {
    ReadHalf,
    forward_rbv(LocalSocketStream, *),
    forward_sync_ref_read,
    forward_as_handle,
    derive_sync_mut_read,
}

#[derive(Debug)]
pub struct WriteHalf(pub(super) Arc<LocalSocketStream>);
multimacro! {
    WriteHalf,
    forward_rbv(LocalSocketStream, *),
    forward_sync_ref_write,
    forward_as_handle,
    derive_sync_mut_write,
}
