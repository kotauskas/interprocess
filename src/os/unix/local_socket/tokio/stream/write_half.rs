use crate::os::unix::udsocket::tokio::WriteHalf as WriteHalfImpl;
use std::{
    fmt::{self, Debug, Formatter},
    pin::Pin,
};

pub struct WriteHalf(pub(super) WriteHalfImpl);
impl WriteHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
        Pin::new(&mut self.0)
    }
}
impl Debug for WriteHalf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("local_socket::WriteHalf").field(&self.0).finish()
    }
}

multimacro! {
    WriteHalf,
    forward_futures_write,
    forward_as_handle,
}
