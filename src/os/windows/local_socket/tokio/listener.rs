use super::LocalSocketStream;
use crate::{
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{
        pipe_mode,
        tokio::{PipeListener as GenericPipeListener, PipeListenerOptionsExt as _},
        PipeListenerOptions, PipeMode,
    },
};
use std::io;

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct LocalSocketListener(PipeListener);
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create_tokio()?;
        Ok(Self(inner))
    }
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.0.accept().await?;
        Ok(LocalSocketStream(inner))
    }
}
