use std::pin::Pin;

impmod! {local_socket::tokio,
    WriteHalf as WriteHalfImpl
}

/// A write half of a Tokio-based local socket stream, obtained by splitting a [`LocalSocketStream`].
///
/// # Examples
/// - [Basic client](https://github.com/kotauskas/interprocess/blob/main/examples/tokio_local_socket/client.rs)
///
/// [`LocalSocketStream`]: struct.LocalSocketStream.html " "
// TODO remove this GitHub link and others like it
pub struct WriteHalf(pub(super) WriteHalfImpl);
impl WriteHalf {
    #[inline]
    fn pinproj(&mut self) -> Pin<&mut WriteHalfImpl> {
        Pin::new(&mut self.0)
    }
}

multimacro! {
    WriteHalf,
    forward_futures_write,
    forward_as_handle,
    forward_debug,
    derive_asraw,
}
