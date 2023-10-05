use crate::os::unix::udsocket::tokio::ReadHalf as ReadHalfImpl;
// TODO compact
pub struct ReadHalf(pub(super) ReadHalfImpl);
multimacro! {
    ReadHalf,
    pinproj_for_unpin(ReadHalfImpl),
    debug_forward_with_custom_name("local_socket::ReadHalf"),
    forward_futures_read,
    forward_as_handle,
}
