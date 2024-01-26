use crate::{
    error::{FromHandleError, ReuniteError},
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{
        pipe_mode::Bytes,
        tokio::{DuplexPipeStream, RecvPipeStream, SendPipeStream},
    },
};
use std::{io, os::windows::prelude::*};

type LocalSocketStreamImpl = DuplexPipeStream<Bytes>;
type RecvHalfImpl = RecvPipeStream<Bytes>;
type SendHalfImpl = SendPipeStream<Bytes>;

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) LocalSocketStreamImpl);
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = LocalSocketStreamImpl::connect(name.inner()).await?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn split(self) -> (RecvHalf, SendHalf) {
        let (r, w) = self.0.split();
        (RecvHalf(r), SendHalf(w))
    }
    #[inline]
    pub fn reunite(rh: RecvHalf, sh: SendHalf) -> Result<Self, ReuniteError<RecvHalf, SendHalf>> {
        LocalSocketStreamImpl::reunite(rh.0, sh.0)
            .map(Self)
            .map_err(|ReuniteError { rh, sh }| ReuniteError { rh: RecvHalf(rh), sh: SendHalf(sh) })
    }
}

impl TryFrom<OwnedHandle> for LocalSocketStream {
    type Error = FromHandleError;

    fn try_from(handle: OwnedHandle) -> Result<Self, Self::Error> {
        match LocalSocketStreamImpl::try_from(handle) {
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
    pinproj_for_unpin(LocalSocketStreamImpl),
    forward_rbv(LocalSocketStreamImpl, &),
    forward_tokio_rw,
    forward_tokio_ref_rw,
    forward_as_handle,
}

pub struct RecvHalf(pub(super) RecvHalfImpl);
multimacro! {
    RecvHalf,
    pinproj_for_unpin(RecvHalfImpl),
    forward_rbv(RecvHalfImpl, &),
    forward_tokio_read,
    forward_tokio_ref_read,
    forward_as_handle,
    forward_debug("local_socket::RecvHalf"),
}

pub struct SendHalf(pub(super) SendHalfImpl);
multimacro! {
    SendHalf,
    pinproj_for_unpin(SendHalfImpl),
    forward_rbv(SendHalfImpl, &),
    forward_tokio_write,
    forward_tokio_ref_write,
    forward_as_handle,
    forward_debug("local_socket::SendHalf"),
}
