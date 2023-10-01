use {
    crate::os::windows::named_pipe::{pipe_mode, tokio::RecvPipeStream},
    std::{
        fmt::{self, Debug, Formatter},
        pin::Pin,
    },
};

type ReadHalfImpl = RecvPipeStream<pipe_mode::Bytes>;

pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}

impl Debug for ReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::WriteHalf").field(&self.0).finish()
    }
}

multimacro! {
    ReadHalf,
    forward_futures_read,
    forward_as_handle,
}
