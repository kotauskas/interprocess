use {
    super::LocalSocketStream,
    crate::{
        local_socket::ToLocalSocketName,
        os::windows::named_pipe::{
            tokio::{
                DuplexBytePipeStream as PipeStream, PipeListener as GenericPipeListener,
                PipeListenerOptionsExt as _,
            },
            PipeListenerOptions, PipeMode,
        },
    },
    std::io,
};

type PipeListener = GenericPipeListener<PipeStream>;

#[derive(Debug)]
pub struct LocalSocketListener {
    inner: PipeListener,
}
impl LocalSocketListener {
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeListenerOptions::new()
            .name(name.into_inner())
            .mode(PipeMode::Bytes)
            .create_tokio()?;
        Ok(Self { inner })
    }
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept().await?;
        Ok(LocalSocketStream { inner })
    }
}
