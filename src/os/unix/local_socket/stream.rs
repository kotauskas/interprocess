use super::name_to_addr;
use crate::local_socket::ToLocalSocketName;
use std::{io, os::unix::net::UnixStream, sync::Arc};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) UnixStream);
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let addr = name_to_addr(name.to_local_socket_name()?)?;
        let inner = UnixStream::connect_addr(&addr)?;
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
    forward_rbv(UnixStream, &),
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
