//! Traits for receiving from IPC channels with message boundaries reliably, without truncation.
//!
//! ## The problem
//! Unlike a byte stream interface, message-mode named pipes preserve boundaries between different write calls, which is
//! what "message boundary" essentially means. Extracting messages by partial reads is an error-prone task, which is why
//! no such interface is exposed by any OS – instead, all messages received from message IPC channels are full messages
//! rather than chunks of messages, which simplifies things to a great degree and is arguably the only proper way of
//! implementing datagram support.
//!
//! There is one pecularity related to this design: you can't just use a buffer with arbitrary length to successfully
//! receive a message. With byte streams, that always works – there either is some data which can be written into that
//! buffer or end of file has been reached, aside from the implied error case which is always a possibility for any kind
//! of I/O. With message streams, however, **there might not always be enough space in a buffer to fetch a whole
//! message**. If the buffer is too small to fetch a message, it won't be written into the buffer, but simply will be
//! ***discarded*** instead. The only way to protect from it being discarded is first checking whether the message fits
//! into the buffer without discarding it and then actually receiving it into a suitably large buffer. In such a case,
//! the message needs an alternate channel besides the buffer to somehow get returned.
//!
//! This brings us to the signature of the `recv` method:
//! ```no_run
//! # use std::io;
//! # type RecvResult = ();
//! # trait Tr {
//! fn recv(&mut self, buf: &mut [u8]) -> io::Result<RecvResult>;
//! # }
//! ```
//! Notice the nested result that's going on here. Setting aside from the `io::Result` part, the "true return value" is
//! [`RecvResult`]. The `Fit(...)` variant here means that the message has been successfully received into the buffer
//! and contains the actual size of the message which has been received. The `Alloc(...)` variant means that the buffer
//! was too small for the message, containing a freshly allocated buffer which is just big enough to fit the message.
//! The usage strategy is to store a buffer, mutably borrow it and pass it to the `recv` function, see if it fits inside
//! the buffer, and if it does not, replace the stored buffer with the new one.
//!
//! The `try_recv` method is used mainly by implementations of `recv`, but can also be called directly. It has the
//! following signature:
//! ```no_run
//! # use std::io;
//! # type TryRecvResult = ();
//! # trait Tr {
//! fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<TryRecvResult>;
//! # }
//! ```
//! The inner [`TryRecvResult`] reports both the size of the message and whether it fit into the buffer or not. If it
//! didn't fit, the buffer is unaffected (unlike with `RecvResult`).
//!
//! ## Platform support
//! The traits are implemented for:
//! - Named pipes on Windows (module `interprocess::os::windows::named_pipe`)
//! - Unix domain pipes, but only on Linux (module `interprocess::os::unix::udsocket`)
//!     - This is because only Linux provides a special flag for `recv` which returns the amount of bytes in the message
//!       regardless of the provided buffer size when peeking.
// TODO redocument for new API
// TODO API for receiving multiple messages

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    future::Future,
    io,
    mem::{self, transmute, MaybeUninit},
    pin::Pin,
    task::{Context, Poll},
};

fn check_second_try_recv_length(expected: usize, found: usize, try_recv: &str) {
    assert_eq!(
        expected, found,
        "\
message at the front of the queue has different length from what was reported by a prior \
.{try_recv}() call"
    )
}
fn check_base_pointer(expected: *const u8, found: *const u8, try_recv: &str, recv: &str) {
    assert_eq!(
        expected, found,
        "\
base pointer of the message slice returned by the second .{try_recv}() call does not match that of \
the internal buffer allocated by the default implementation of .{recv}()"
    )
}
fn panic_try_recv_retcon() -> ! {
    panic!(
        "\
try_recv() returned TryRecvResult::Failed for a buffer of a size that it reported was sufficient"
    )
}

/// Receiving from IPC channels with message boundaries reliably, without truncation.
///
/// See the [module-level documentation](self) for more.
pub trait ReliableRecvMsg {
    /// Attempts to receive one message from the stream using the given buffer, returning a borrowed
    /// message, which, depending on the variant of [`TryRecvResult`], either did fit or did not fit
    /// into the provided buffer.
    ///
    /// The buffer in this case is used solely as a scratchpad – the returned message slice
    /// constitutes this method's useful output – although it is incorrect, given a return value of
    /// `TryRecvResult::Fit(msg)`, for `buf.as_ptr() == msg.as_ptr()` to be false.
    ///
    /// If the operation could not be completed for OS reasons, an error from the outermost `Result`
    /// is returned.
    fn try_recv<'buf>(&mut self, buf: &'buf mut [MaybeUninit<u8>]) -> io::Result<TryRecvResult<'buf>>;

    /// Receives one message from the stream using the given buffer, returning either a borrowed
    /// slice of the message, a bigger buffer if the one provided was too small, or an error in the
    /// outermost `Result` if the operation could not be completed for OS reasons.
    fn recv<'buf>(&mut self, buf: &'buf mut [MaybeUninit<u8>]) -> io::Result<RecvResult<'buf>> {
        match self.try_recv(buf)? {
            TryRecvResult::Fit(msg) => Ok(RecvResult::Fit(msg)),
            TryRecvResult::EndOfStream => Ok(RecvResult::EndOfStream),
            TryRecvResult::Failed(size) => {
                let mut alloc = Vec::with_capacity(size);

                let msg = match self.try_recv(alloc.spare_capacity_mut())? {
                    TryRecvResult::Fit(msg) => msg,
                    #[rustfmt::skip] TryRecvResult::Failed(..) => panic_try_recv_retcon(),
                    TryRecvResult::EndOfStream => return Ok(RecvResult::EndOfStream),
                };

                check_second_try_recv_length(size, msg.len(), "try_recv");
                check_base_pointer(msg.as_ptr(), alloc.as_ptr(), "try_recv", "recv");

                unsafe {
                    alloc.set_len(size);
                }
                Ok(RecvResult::Alloc(alloc))
            }
        }
    }
}

/// Implementation of asynchronously receiving from IPC channels with message boundaries reliably, without truncation.
///
/// See the [module-level documentation](self) for more.
pub trait AsyncReliableRecvMsg {
    // TODO redocument
    /// Polls a future that attempts to receive one message from the stream into the specified buffer, returning the
    /// size of the message, which, depending on whether it was in the `Ok` or `Err` variant, either did fit or did not
    /// fit into the provided buffer, respectively; if the operation could not be completed for OS reasons, an error
    /// from the outermost `Result` is returned.
    fn poll_try_recv<'buf>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &'buf mut [MaybeUninit<u8>],
    ) -> Poll<io::Result<TryRecvResult<'buf>>>;

    /// Polls a future that aeceives one message from the stream into the specified buffer, returning either the size of
    /// the message written, a bigger buffer if the one provided was too small, or an error in the outermost `Result` if
    /// the operation could not be completed for OS reasons.
    fn poll_recv<'buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &'buf mut [MaybeUninit<u8>],
    ) -> Poll<io::Result<RecvResult<'buf>>> {
        let Poll::Ready(trr) = self.as_mut().poll_try_recv(cx, buf) else {
            return Poll::Pending;
        };
        Poll::Ready(match trr? {
            TryRecvResult::Fit(size) => Ok(RecvResult::Fit(size)),
            TryRecvResult::EndOfStream => Ok(RecvResult::EndOfStream),
            TryRecvResult::Failed(size) => {
                let mut alloc = Vec::with_capacity(size);
                let Poll::Ready(trr) = self.as_mut().poll_try_recv(cx, alloc.spare_capacity_mut()) else {
                    // This isn't supposed to be hit normally, since the buffer would be wasted then.
                    return Poll::Pending;
                };
                let msg = match trr? {
                    TryRecvResult::Fit(msg) => msg,
                    #[rustfmt::skip] TryRecvResult::Failed(..) => panic_try_recv_retcon(),
                    TryRecvResult::EndOfStream => return Poll::Ready(Ok(RecvResult::EndOfStream)),
                };

                check_second_try_recv_length(size, msg.len(), "poll_try_recv");
                check_base_pointer(msg.as_ptr(), alloc.as_ptr(), "poll_try_recv", "poll_recv");

                unsafe {
                    alloc.set_len(size);
                }
                Ok(RecvResult::Alloc(alloc))
            }
        })
    }
}

/// Futures for asynchronously receiving from IPC channels with message boundaries reliably, without truncation.
///
/// See the [module-level documentation](self) for more.
pub trait AsyncReliableRecvMsgExt: AsyncReliableRecvMsg {
    // TODO redocument
    /// Asynchronously receives one message from the stream into the specified buffer, returning either the size of the
    /// message written, a bigger buffer if the one provided was too small, or an error in the outermost `Result` if the
    /// operation could not be completed for OS reasons.
    #[inline]
    fn recv<'io, 'buf>(&'io mut self, buf: &'buf mut [MaybeUninit<u8>]) -> Recv<'io, 'buf, Self>
    where
        Self: Unpin,
    {
        Recv {
            recver: Some(Pin::new(self)),
            buf,
        }
    }

    /// Asynchronously attempts to receive one message from the stream into the specified buffer, returning the size of
    /// the message, which, depending on whether it was in the `Ok` or `Err` variant, either did fit or did not fit into
    /// the provided buffer, respectively; if the operation could not be completed for OS reasons, an error from the
    /// outermost `Result` is returned.
    #[inline]
    fn try_recv<'io, 'buf>(&'io mut self, buf: &'buf mut [MaybeUninit<u8>]) -> TryRecv<'io, 'buf, Self>
    where
        Self: Unpin,
    {
        TryRecv {
            recver: Some(Pin::new(self)),
            buf,
        }
    }
}
impl<T: AsyncReliableRecvMsg> AsyncReliableRecvMsgExt for T {}

static REPOLL_ERR: &str = "attempt to poll a future which has already completed";

/// Future type returned by [`.recv()`](AsyncReliableRecvMsgExt::recv).
#[derive(Debug)]
pub struct Recv<'io, 'buf, T: ?Sized> {
    recver: Option<Pin<&'io mut T>>,
    buf: &'buf mut [MaybeUninit<u8>],
}
impl<'buf, T: AsyncReliableRecvMsg + Unpin + ?Sized> Future for Recv<'_, 'buf, T> {
    type Output = io::Result<RecvResult<'buf>>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Recv {
            recver: self_recver,
            buf: self_buf,
        } = self.get_mut();
        let mut recver = self_recver.take().expect(REPOLL_ERR);
        let buf = mem::take(self_buf);
        let ret = match recver.as_mut().poll_recv(cx, buf) {
            Poll::Ready(ret) => {
                match ret {
                    Ok(RecvResult::Fit(msg)) => {
                        let msg = unsafe {
                            // SAFETY: `self.buf` at this point is empty, and the slice we're
                            // getting here is a direct descendant (reborrow) of that slice, which
                            // means that this is the only instance of that slice in existence at
                            // this level in the borrow stack (I *think* that's how this sort of
                            // thing is called). What we're essentially doing here is avoiding a
                            // reborrow of `self` (which would infect it with an anonymous lifetime
                            // we can't name because `Future::Output` is not a GAT) and instead
                            // "unearthing" the 'buf lifetime within the return value.
                            //
                            // pizzapants184 told me on RPLCS (in #dark-arts) that Polonius would
                            // be smart enough to allow this in safe code (and also kindly provided
                            // me with a snippet which this whole function is based on). I haven't
                            // tried using the `polonius_the_crab` crate because that's a whole
                            // extra dependency, but it should be doable with that crate if need be.
                            transmute::<&'_ [u8], &'buf [u8]>(msg)
                        };
                        Ok(RecvResult::Fit(msg))
                    }
                    Ok(els) => Ok(els.make_static().unwrap()),
                    Err(e) => Err(e),
                }
            }
            Poll::Pending => {
                *self_recver = Some(recver);
                *self_buf = buf;
                return Poll::Pending;
            }
        };
        Poll::Ready(ret)
    }
}
/// Future type returned by [`.try_recv()`](AsyncReliableRecvMsgExt::try_recv).
#[derive(Debug)]
pub struct TryRecv<'io, 'buf, T: ?Sized> {
    recver: Option<Pin<&'io mut T>>,
    buf: &'buf mut [MaybeUninit<u8>],
}
impl<'buf, T: AsyncReliableRecvMsg + Unpin + ?Sized> Future for TryRecv<'_, 'buf, T> {
    type Output = io::Result<TryRecvResult<'buf>>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let TryRecv {
            recver: self_recver,
            buf: self_buf,
        } = self.get_mut();
        let mut recver = self_recver.take().expect(REPOLL_ERR);
        let buf = mem::take(self_buf);
        let ret = match recver.as_mut().poll_try_recv(cx, buf) {
            Poll::Ready(ret) => {
                match ret {
                    Ok(TryRecvResult::Fit(msg)) => {
                        let msg = unsafe {
                            // SAFETY: same as above.
                            transmute::<&'_ [u8], &'buf [u8]>(msg)
                        };
                        Ok(TryRecvResult::Fit(msg))
                    }
                    Ok(els) => Ok(els.make_static().unwrap()),
                    Err(e) => Err(e),
                }
            }
            Poll::Pending => {
                *self_recver = Some(recver);
                *self_buf = buf;
                return Poll::Pending;
            }
        };
        Poll::Ready(ret)
    }
}

/// Marker error indicating that a datagram write operation failed because the amount of bytes which were actually
/// written as reported by the operating system was smaller than the size of the message which was requested to be
/// written.
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
pub enum RecvResult<'buf> {
    /// The message successfully fit into the provided buffer.
    Fit(&'buf [u8]),
    /// The message didn't fit into the provided buffer and the given bigger buffer has been
    /// allocated which it's been written to instead.
    Alloc(Vec<u8>),
    /// Indicates that the message stream has ended and no more messages will be received.
    EndOfStream,
}
impl RecvResult<'_> {
    // TODO remove all mentions of borrow_to_size
    /// Extends the lifetime to `'static` if the variant is not `Fit`.
    #[inline]
    pub fn make_static(self) -> Result<RecvResult<'static>, Self> {
        match self {
            Self::Fit(..) => Err(self),
            Self::Alloc(alloc) => Ok(RecvResult::Alloc(alloc)),
            Self::EndOfStream => Ok(RecvResult::EndOfStream),
        }
    }
}
impl AsRef<[u8]> for RecvResult<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Fit(msg) => msg,
            Self::Alloc(buf) => buf,
            Self::EndOfStream => &[],
        }
    }
}

/// Result type for `.try_recv()` methods.
///
/// `Ok` indicates that the message fits in the provided buffer and was successfully received, `Err` indicates that it
/// doesn't and hence wasn't written into the buffer. Both variants' payload is the total size of the message.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TryRecvResult<'buf> {
    /// The message successfully fit into the provided buffer.
    Fit(&'buf [u8]),
    /// The message didn't fit into the provided buffer and hasn't been written anywhere or taken
    /// off any queue.
    Failed(usize),
    /// Indicates that the message stream has ended and no more messages will be received.
    #[default]
    EndOfStream,
}
impl TryRecvResult<'_> {
    // TODO remove all mentions of borrow_to_size
    /// Extends the lifetime to `'static` if the variant is not `Fit`.
    #[inline]
    pub fn make_static(self) -> Result<TryRecvResult<'static>, Self> {
        match self {
            Self::Fit(..) => Err(self),
            Self::Failed(sz) => Ok(TryRecvResult::Failed(sz)),
            Self::EndOfStream => Ok(TryRecvResult::EndOfStream),
        }
    }
}
impl AsRef<[u8]> for TryRecvResult<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Fit(msg) => msg,
            Self::Failed(..) | Self::EndOfStream => &[],
        }
    }
}
