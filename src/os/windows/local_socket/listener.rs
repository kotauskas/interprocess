use super::LocalSocketStream;
use crate::{
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{pipe_mode::Bytes, PipeListener as GenericPipeListener, PipeListenerOptions, PipeMode},
};
use std::io;

type PipeListener = GenericPipeListener<Bytes, Bytes>;

#[derive(Debug)]
pub struct LocalSocketListener(PipeListener);
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create()?;
        Ok(Self(inner))
    }
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.0.accept()?;
        Ok(LocalSocketStream(inner))
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}
forward_into_handle!(LocalSocketListener);
