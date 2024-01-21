// TODO reunite

use crate::{
    error::{FromHandleError, ReuniteError},
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{
        pipe_mode::Bytes,
        tokio::{DuplexPipeStream, RecvPipeStream, ReuniteError as InnerReuniteError, SendPipeStream},
    },
};
use std::{io, os::windows::prelude::*};

type LocalSocketStreamImpl = DuplexPipeStream<Bytes>;
type ReadHalfImpl = RecvPipeStream<Bytes>;
type WriteHalfImpl = SendPipeStream<Bytes>;

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) LocalSocketStreamImpl);
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = LocalSocketStreamImpl::connect(name.inner()).await?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
    #[inline]
    pub fn reunite(rh: ReadHalf, sh: WriteHalf) -> Result<Self, ReuniteError<ReadHalf, WriteHalf>> {
        match LocalSocketStreamImpl::reunite(rh.0, sh.0) {
            Ok(inner) => Ok(Self(inner)),
            Err(InnerReuniteError { rh, sh }) => Err(ReuniteError {
                rh: ReadHalf(rh),
                sh: WriteHalf(sh),
            }),
        }
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

pub struct ReadHalf(pub(super) ReadHalfImpl);
multimacro! {
    ReadHalf,
    pinproj_for_unpin(ReadHalfImpl),
    forward_rbv(ReadHalfImpl, &),
    forward_tokio_read,
    forward_tokio_ref_read,
    forward_as_handle,
    forward_debug("local_socket::ReadHalf"),
}

pub struct WriteHalf(pub(super) WriteHalfImpl);
multimacro! {
    WriteHalf,
    pinproj_for_unpin(WriteHalfImpl),
    forward_rbv(WriteHalfImpl, &),
    forward_tokio_write,
    forward_tokio_ref_write,
    forward_as_handle,
    forward_debug("local_socket::WriteHalf"),
}
