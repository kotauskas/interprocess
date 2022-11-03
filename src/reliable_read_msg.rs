use {
    super::Sealed,
    std::{
        error::Error,
        fmt::{self, Display, Formatter},
        io,
    },
};

/// Reading from named pipes with message boundaries reliably, without truncation.
///
/// ## The problem
/// Unlike a byte stream interface, message-mode named pipes preserve boundaries between different write calls, which is what "message boundary" essentially means. Extracting messages by partial reads is an error-prone task, which is why no such interface is exposed by the operating system – instead, all messages read from a named pipe stream are full messages rather than chunks of messages, which simplifies things to a great degree and is arguably the only proper way of implementing datagram support.
///
/// There is one pecularity related to this design: you can't just use a buffer with arbitrary length to successfully read a message. With byte streams, that always works – there either is some data which can be written into that buffer or end of file has been reached, aside from the implied error case which is always a possibility for any kind of I/O. With message streams, however, **there might not always be enough space in a buffer to fetch a whole message**. If the buffer is too small to fetch a message, it won't be written into the buffer, but simply will be ***discarded*** instead. The only way to protect from it being discarded is first checking whether the message fits into the buffer without discarding it and then actually reading it into a suitably large buffer. In such a case, the message needs an alternate channel besides the buffer to somehow get returned.
///
/// This brings the discussion specifically to the signature of the `read_msg` method:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>>;
/// # }
/// ```
/// Setting aside from the `io::Result` part, the "true return value" is `Result<usize, Vec<u8>>`. The `Ok(...)` variant here means that the message has been successfully read into the buffer and contains the actual size of the message which has been read. The `Err(...)` variant means that the buffer was too small for the message, containing a freshly allocated buffer which is just big enough to fit the message. The usage strategy is to store a buffer, mutably borrow it and pass it to the `read_msg` function, see if it fits inside the buffer, and if it does not, replace the stored buffer with the new one.
///
/// The `try_read_msg` method is a convenience function used mainly by implementations of `read_msg` to determine whether it's required to allocate a new buffer or not. It has the following signature:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>>;
/// # }
/// ```
/// While it may seem strange how the nested `Result` returns the same type in `Ok` and `Err`, it does this for a semantic reason: the `Ok` variant means that the message was successfully read into the buffer while `Err` means the opposite – that the message was too big – and returns the size which the buffer needs to have.
///
/// ## Platform support
/// The trait is implemented for:
/// - Named pipes on Windows (module `interprocess::os::windows::named_pipe`)
/// - Unix domain pipes, but only on Linux (module `interprocess::os::unix::udsocket`)
///     - This is because only Linux provides a special flag for `recv` which returns the amount of bytes in the message regardless of the provided buffer size when peeking.
pub trait ReliableReadMsg: Sealed {
    /// Reads one message from the stream into the specified buffer, returning either the size of the message written, a bigger buffer if the one provided was too small, or an error in the outermost `Result` if the operation could not be completed for OS reasons.
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>>;

    /// Attempts to read one message from the stream into the specified buffer, returning the size of the message, which, depending on whether it was in the `Ok` or `Err` variant, either did fit or did not fit into the provided buffer, respectively; if the operation could not be completed for OS reasons, an error from the outermost `Result` is returned.
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>>;
}

/// Marker error indicating that a datagram write operation failed because the amount of bytes which were actually written as reported by the operating system was smaller than the size of the message which was requested to be written.
///
/// Always emitted with the `ErrorKind::Other` error type.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialMsgWriteError;
impl Display for PartialMsgWriteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("message write operation wrote less than the size of the message")
    }
}
impl Error for PartialMsgWriteError {}
