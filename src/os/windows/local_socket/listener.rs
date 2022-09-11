use {
    super::LocalSocketStream,
    crate::{
        local_socket::ToLocalSocketName,
        os::windows::named_pipe::{
            DuplexBytePipeStream as PipeStream, PipeListener as GenericPipeListener,
            PipeListenerOptions, PipeMode,
        },
    },
    std::{
        fmt::{self, Debug, Formatter},
        io,
    },
};

type PipeListener = GenericPipeListener<PipeStream>;

pub struct LocalSocketListener {
    inner: PipeListener,
}
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create()?;
        Ok(Self { inner })
    }
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept()?;
        Ok(LocalSocketStream { inner })
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}
impl Debug for LocalSocketListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("LocalSocketListener")
    }
}
