mod read_half;
pub use read_half::*;

mod write_half;
pub use write_half::*;
// TODO reunite

use crate::{
    error::FromHandleError,
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{pipe_mode, tokio::DuplexPipeStream},
};
use std::{io, os::windows::prelude::*, pin::Pin};

type StreamImpl = DuplexPipeStream<pipe_mode::Bytes>;

#[derive(Debug)]
pub struct LocalSocketStream(pub(super) StreamImpl);
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
    fn pinproj(&mut self) -> Pin<&mut StreamImpl> {
        Pin::new(&mut self.0)
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

// TODO I/O by ref, including Tokio traits
multimacro! {
    LocalSocketStream,
    forward_futures_rw,
    forward_as_handle,
}
