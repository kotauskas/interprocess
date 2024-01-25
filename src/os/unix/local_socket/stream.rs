use super::name_to_addr;
use crate::{
    error::{FromHandleError, ReuniteError},
    local_socket::LocalSocketName,
};
use std::{io, os::unix::net::UnixStream, sync::Arc};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) UnixStream);
impl LocalSocketStream {
    pub fn connect(name: LocalSocketName<'_>) -> io::Result<Self> {
        UnixStream::connect_addr(&name_to_addr(name)?).map(Self)
    }
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let arc = Arc::new(self);
        (ReadHalf(Arc::clone(&arc)), WriteHalf(arc))
    }
    #[inline]
    pub fn reunite(rh: ReadHalf, sh: WriteHalf) -> Result<Self, ReuniteError<ReadHalf, WriteHalf>> {
        if !Arc::ptr_eq(&rh.0, &sh.0) {
            return Err(ReuniteError { rh, sh });
        }
        let inner = Arc::into_inner(sh.0).unwrap();
        drop(rh);
        Ok(Self(inner))
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
