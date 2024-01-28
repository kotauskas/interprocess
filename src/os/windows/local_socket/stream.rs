use crate::{
    error::{FromHandleError, ReuniteError},
    local_socket::LocalSocketName,
    os::windows::named_pipe::{pipe_mode::Bytes, DuplexPipeStream, RecvPipeStream, SendPipeStream},
};
use std::{io, os::windows::prelude::*};

type StreamImpl = DuplexPipeStream<Bytes>;
type RecvHalfImpl = RecvPipeStream<Bytes>;
type SendHalfImpl = SendPipeStream<Bytes>;

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) StreamImpl);
impl LocalSocketStream {
    pub fn connect(name: LocalSocketName<'_>) -> io::Result<Self> {
        if name.is_namespaced() {
            StreamImpl::connect_with_prepend(name.inner(), None)
        } else {
            StreamImpl::connect(name.inner())
        }
        .map(Self)
    }
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    #[inline]
    pub fn split(self) -> (RecvHalf, SendHalf) {
        let (r, w) = self.0.split();
        (RecvHalf(r), SendHalf(w))
    }
    #[inline]
    pub fn reunite(rh: RecvHalf, sh: SendHalf) -> Result<Self, ReuniteError<RecvHalf, SendHalf>> {
        StreamImpl::reunite(rh.0, sh.0)
            .map(Self)
            .map_err(|ReuniteError { rh, sh }| ReuniteError {
                rh: RecvHalf(rh),
                sh: SendHalf(sh),
            })
    }
}

impl From<LocalSocketStream> for OwnedHandle {
    fn from(s: LocalSocketStream) -> Self {
        // The outer local socket interface has receive and send halves and is always duplex in the
        // unsplit type, so a split pipe stream can never appear here.
        s.0.try_into()
            .expect("split named pipe stream inside `LocalSocketStream`")
    }
}
impl TryFrom<OwnedHandle> for LocalSocketStream {
    type Error = FromHandleError;

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        match StreamImpl::try_from(handle) {
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
    forward_rbv(StreamImpl, &),
    forward_sync_ref_rw, // The thunking already happens inside.
    forward_as_handle,
    forward_try_clone,
    derive_sync_mut_rw,
}

#[derive(Debug)]
pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
    RecvHalf,
    forward_rbv(RecvHalfImpl, &),
    forward_sync_ref_read,
    forward_as_handle,
    derive_sync_mut_read,
}

#[derive(Debug)]
pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
    SendHalf,
    forward_rbv(SendHalfImpl, &),
    forward_sync_ref_write,
    forward_as_handle,
    derive_sync_mut_write,
}
