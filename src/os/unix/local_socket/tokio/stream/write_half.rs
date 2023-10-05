use crate::os::unix::udsocket::tokio::WriteHalf as WriteHalfImpl;
pub struct WriteHalf(pub(super) WriteHalfImpl);
multimacro! {
    WriteHalf,
    pinproj_for_unpin(WriteHalfImpl),
    debug_forward_with_custom_name("local_socket::WriteHalf"),
    forward_futures_write,
    forward_as_handle,
}
