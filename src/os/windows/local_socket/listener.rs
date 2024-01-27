use super::LocalSocketStream;
use crate::{local_socket::ToLocalSocketName, os, os::windows::named_pipe::{
    pipe_mode::Bytes, PipeListener as GenericPipeListener, PipeListenerOptions,
}};
use std::{
    io,
    path::{Path, PathBuf},
};

type PipeListener = GenericPipeListener<Bytes, Bytes>;

#[derive(Debug)]
pub struct LocalSocketListener(PipeListener);
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>, security_attributes: Option<os::windows::security_descriptor::SecurityAttributes>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let path = Path::new(name.inner());
        let mut options = PipeListenerOptions::new().security_attributes(security_attributes.unwrap_or_default());
        options.path = if name.is_namespaced() {
            // PERF this allocates twice
            [Path::new(r"\\.\pipe\"), path].iter().collect::<PathBuf>().into()
        } else {
            path.into()
        };
        options.create().map(Self)
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
