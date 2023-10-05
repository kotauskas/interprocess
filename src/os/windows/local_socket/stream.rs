use crate::{
    error::FromHandleError,
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{pipe_mode::Bytes, DuplexPipeStream, RecvPipeStream, SendPipeStream},
};
use std::{io, os::windows::prelude::*};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) DuplexPipeStream<Bytes>);
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = DuplexPipeStream::connect(name.inner())?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
}

impl From<LocalSocketStream> for OwnedHandle {
    fn from(s: LocalSocketStream) -> Self {
        // The outer local socket interface has read and write halves and is always duplex in the
        // unsplit type, so a split pipe stream can never appear here.
        s.try_into()
            .expect("split named pipe stream inside `LocalSocketStream`")
    }
}
impl TryFrom<OwnedHandle> for LocalSocketStream {
    type Error = FromHandleError;

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        match DuplexPipeStream::try_from(handle) {
            Ok(s) => Ok(Self(s)),
            Err(e) => Err(FromHandleError {
                details: Default::default(),
                cause: Some(e.details.into()),
                source: e.source,
            }),
        }
    }
}

multimacro! {
    LocalSocketStream,
    forward_sync_ref_rw, // The thunking already happens inside.
    forward_as_handle,
    derive_sync_mut_rw,
}

#[derive(Debug)]
pub struct ReadHalf(pub(super) RecvPipeStream<Bytes>);
multimacro! {
    ReadHalf,
    forward_sync_ref_read,
    forward_as_handle,
    derive_sync_mut_read,
}

#[derive(Debug)]
pub struct WriteHalf(pub(super) SendPipeStream<Bytes>);
multimacro! {
    WriteHalf,
    forward_sync_ref_write,
    forward_as_handle,
    derive_sync_mut_write,
}
