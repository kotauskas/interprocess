use std::pin::Pin;

impmod! {local_socket::tokio,
    ReadHalf as ReadHalfImpl
}

/// A read half of a Tokio-based local socket stream, obtained by splitting a
/// [`LocalSocketStream`](super::LocalSocketStream).
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
pub struct ReadHalf(pub(super) ReadHalfImpl);
impl ReadHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut ReadHalfImpl> {
        Pin::new(&mut self.0)
    }
}

multimacro! {
    ReadHalf,
    forward_futures_read,
    forward_as_handle,
    forward_debug,
    derive_asraw,
}
