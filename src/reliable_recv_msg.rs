use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
};

/// Receiving from IPC channels with message boundaries reliably, without truncation.
///
/// ## The problem
/// Unlike a byte stream interface, message-mode named pipes preserve boundaries between different write calls, which is what "message boundary" essentially means. Extracting messages by partial reads is an error-prone task, which is why no such interface is exposed by any OS – instead, all messages received from message IPC channels are full messages rather than chunks of messages, which simplifies things to a great degree and is arguably the only proper way of implementing datagram support.
///
/// There is one pecularity related to this design: you can't just use a buffer with arbitrary length to successfully receive a message. With byte streams, that always works – there either is some data which can be written into that buffer or end of file has been reached, aside from the implied error case which is always a possibility for any kind of I/O. With message streams, however, **there might not always be enough space in a buffer to fetch a whole message**. If the buffer is too small to fetch a message, it won't be written into the buffer, but simply will be ***discarded*** instead. The only way to protect from it being discarded is first checking whether the message fits into the buffer without discarding it and then actually receiving it into a suitably large buffer. In such a case, the message needs an alternate channel besides the buffer to somehow get returned.
///
/// This brings the discussion specifically to the signature of the `recv` method:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult>;
/// # }
/// ```
/// Notice the nested result that's going on here. Setting aside from the `io::Result` part, the "true return value" is [`RecvResult`]. The `Fit(...)` variant here means that the message has been successfully received into the buffer and contains the actual size of the message which has been received. The `Alloc(...)` variant means that the buffer was too small for the message, containing a freshly allocated buffer which is just big enough to fit the message. The usage strategy is to store a buffer, mutably borrow it and pass it to the `recv` function, see if it fits inside the buffer, and if it does not, replace the stored buffer with the new one.
///
/// The `try_recv` method is used mainly by implementations of `recv`, but can also be called directly. It has the following signature:
/// ```no_run
/// # use std::io;
/// # trait Tr {
/// fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult>;
/// # }
/// ```
/// The inner [`TryRecvResult`] reports both the size of the message and whether it fit into the buffer or not. If it didn't fit, the buffer is unaffected (unlike with `RecvResult`).
///
/// ## Platform support
/// The trait is implemented for:
/// - Named pipes on Windows (module `interprocess::os::windows::named_pipe`)
/// - Unix domain pipes, but only on Linux (module `interprocess::os::unix::udsocket`)
///     - This is because only Linux provides a special flag for `recv` which returns the amount of bytes in the message regardless of the provided buffer size when peeking.
pub trait ReliableRecvMsg {
    /// Receives one message from the stream into the specified buffer, returning either the size of the message written, a bigger buffer if the one provided was too small, or an error in the outermost `Result` if the operation could not be completed for OS reasons.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult>;

    /// Attempts to receive one message from the stream into the specified buffer, returning the size of the message, which, depending on whether it was in the `Ok` or `Err` variant, either did fit or did not fit into the provided buffer, respectively; if the operation could not be completed for OS reasons, an error from the outermost `Result` is returned.
    fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult>;
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

/// Result type for `.recv()` methods.
#[derive(Clone, Debug)]
pub enum RecvResult {
    /// Indicates that the message successfully fit into the provided buffer.
    Fit(usize),
    /// Indicates that it didn't fit into the provided buffer and contains a new, bigger buffer which it was written to instead.
    Alloc(Vec<u8>),
}
impl RecvResult {
    /// Returns the size of the message.
    #[inline]
    pub fn size(&self) -> usize {
        match self {
            Self::Fit(s) => *s,
            Self::Alloc(v) => v.len(),
        }
    }
    /// Returns whether the message fit into the buffer or had to have been put into a new one.
    #[inline]
    pub fn fit(&self) -> bool {
        matches!(self, Self::Fit(..))
    }
    /// Converts to a `Result<usize, Vec<u8>>`, where `Ok` represents `Fit` and `Err` represents `Alloc`.
    #[inline]
    pub fn into_result(self) -> Result<usize, Vec<u8>> {
        match self {
            Self::Fit(f) => Ok(f),
            Self::Alloc(a) => Err(a),
        }
    }
}
impl From<RecvResult> for Result<usize, Vec<u8>> {
    /// See `.into_result()`.
    fn from(x: RecvResult) -> Self {
        x.into_result()
    }
}

/// Result type for `.try_recv()` methods.
///
/// `Ok` indicates that the message fits in the provided buffer and was successfully received, `Err` indicates that it doesn't and hence wasn't written into the buffer. Both variants' payload is the total size of the message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TryRecvResult {
    /// The size of the message.
    pub size: usize,
    /// Whether the message was written to the buffer and taken off the OS queue or not.
    pub fit: bool,
}
impl TryRecvResult {
    /// Converts to a `Result<usize, usize>`, where `Ok` represents `fit = true` and `Err` represents `fit = false`.
    #[inline(always)]
    pub fn to_result(self) -> Result<usize, usize> {
        match (self.size, self.fit) {
            (s, true) => Ok(s),
            (s, false) => Err(s),
        }
    }
}
impl From<TryRecvResult> for Result<usize, usize> {
    /// See `.into_result()`.
    fn from(x: TryRecvResult) -> Self {
        x.to_result()
    }
}
