use {
    crate::os::unix::udsocket::tokio::ReadHalf as ReadHalfImpl,
    futures_io::AsyncRead,
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, IoSliceMut},
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}
impl Debug for ReadHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::ReadHalf").field(&self.0).finish()
    }
}
multimacro! {
    ReadHalf,
    forward_futures_read,
    forward_as_handle,
}
