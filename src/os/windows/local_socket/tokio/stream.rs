// TODO reunite

use crate::{
    error::FromHandleError,
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{
        pipe_mode::Bytes,
        tokio::{DuplexPipeStream, RecvPipeStream, SendPipeStream},
    },
};
use std::{io, os::windows::prelude::*, pin::Pin};

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) DuplexPipeStream<Bytes>);
impl LocalSocketStream {
    pub async fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = DuplexPipeStream::connect(name.inner()).await?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (r, w) = self.0.split();
        (ReadHalf(r), WriteHalf(w))
    }
    pub fn reunite(rh: ReadHalf, wh: WriteHalf) -> io::Result<Self> {
        match DuplexPipeStream::reunite(rh.0, wh.0) {
            Ok(inner) => Ok(Self(inner)),
            Err(_) => todo!(),
        }
    }
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut DuplexPipeStream<Bytes>> {
        Pin::new(&mut self.0)
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

// TODO I/O by ref, including Tokio traits
multimacro! {
    LocalSocketStream,
    forward_futures_rw,
    forward_as_handle,
}

pub struct ReadHalf(pub(super) RecvPipeStream<Bytes>);
impl ReadHalf {
    fn pinproj(&mut self) -> Pin<&mut RecvPipeStream<Bytes>> {
        Pin::new(&mut self.0)
    }
}
multimacro! {
    ReadHalf,
    forward_futures_read,
    forward_as_handle,
    forward_debug("local_socket::ReadHalf"),
}

pub struct WriteHalf(pub(super) SendPipeStream<Bytes>);
impl WriteHalf {
    fn pinproj(&mut self) -> Pin<&mut SendPipeStream<Bytes>> {
        Pin::new(&mut self.0)
    }
}
multimacro! {
    WriteHalf,
    forward_futures_write,
    forward_as_handle,
    forward_debug("local_socket::WriteHalf"),
}
