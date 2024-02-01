use super::*;
use crate::os::windows::downgrade_eof;
use recvmsg::{prelude::*, NoAddrBuf, RecvMsg, RecvResult};
use windows_sys::Win32::Foundation::ERROR_MORE_DATA;

pub(crate) const DISCARD_BUF_SIZE: usize = {
    // Debug builds are more prone to stack explosions.
    if cfg!(debug_assertions) {
        512
    } else {
        4096
    }
};

impl RawPipeStream {
    #[track_caller]
    fn discard_msg(&self) -> io::Result<()> {
        let _guard = self.concurrency_detector.lock();

        let mut buf = [MaybeUninit::uninit(); DISCARD_BUF_SIZE];
        let fh = self.file_handle();
        loop {
            match downgrade_eof(fh.read(&mut buf)) {
                Ok(..) => break Ok(()),
                Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => {}
                Err(e) => break Err(e),
            }
        }
    }

    #[track_caller]
    fn recv_msg(&self, buf: &mut MsgBuf<'_>) -> io::Result<RecvResult> {
        let _guard = self.concurrency_detector.lock();

        buf.set_fill(0);
        buf.has_msg = false;
        let mut more_data = true;
        let mut partial = false;
        let mut spilled = false;
        let fh = self.file_handle();

        while more_data {
            let slice = buf.unfilled_part();
            if slice.is_empty() {
                match buf.grow() {
                    Ok(()) => {
                        spilled = true;
                        debug_assert!(
                            !buf.unfilled_part().is_empty(),
                            "successful buffer growth did not yield additional capacity"
                        );
                        continue;
                    }
                    Err(e) => {
                        if more_data {
                            // A partially successful partial receive must result in the rest of the
                            // message being discarded.
                            let _ = self.discard_msg();
                        }
                        return Ok(RecvResult::QuotaExceeded(e));
                    }
                }
            }

            let rslt = fh.read(slice);
            more_data = false;

            let incr = match decode_eof(rslt) {
                Ok(incr) => incr,
                Err(e) if e.raw_os_error() == Some(ERROR_MORE_DATA as _) => {
                    more_data = true;
                    partial = true;
                    slice.len()
                }
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                    buf.set_fill(0);
                    return Ok(RecvResult::EndOfStream);
                }
                Err(e) => {
                    if partial {
                        // This is irrelevant to normal operation of downstream
                        // programs, but still makes them easier to debug.
                        let _ = self.discard_msg();
                    }
                    return Err(e);
                }
            };
            #[allow(clippy::arithmetic_side_effects)] // this cannot panic due to the isize limit
            unsafe {
                // SAFETY: this one is on Windows
                buf.advance_init_and_set_fill(buf.len_filled() + incr)
            };
        }
        buf.has_msg = true;
        Ok(if spilled {
            RecvResult::Spilled
        } else {
            RecvResult::Fit
        })
    }
}

/// Interacts with [concurrency prevention](#concurrency-prevention).
impl<Sm: PipeModeTag> RecvMsg for &PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(
        &mut self,
        buf: &mut MsgBuf<'_>,
        _: Option<&mut NoAddrBuf>,
    ) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf)
    }
}
/// Interacts with [concurrency prevention](#concurrency-prevention).
impl<Sm: PipeModeTag> RecvMsg for PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(
        &mut self,
        buf: &mut MsgBuf<'_>,
        _: Option<&mut NoAddrBuf>,
    ) -> io::Result<RecvResult> {
        (&*self).recv_msg(buf, None)
    }
}
