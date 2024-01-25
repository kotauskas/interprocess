use super::*;
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
    fn discard_msg(&self) -> io::Result<()> {
        // TODO not delegate to recv_msg
        use RecvResult::*;
        let mut bufbak = [MaybeUninit::uninit(); DISCARD_BUF_SIZE];
        let mut buf = MsgBuf::from(&mut bufbak[..]);
        buf.quota = Some(0);
        loop {
            match self.recv_msg_impl(&mut buf, false)? {
                EndOfStream | Fit => break,
                QuotaExceeded(..) => {
                    // Because discard = false makes sure that discard_msg() isn't recursed into,
                    // we have to manually reset the buffer into a workable state – by discarding
                    // the received data, that is.
                    buf.set_fill(0);
                }
                Spilled => unreachable!(),
            }
        }
        Ok(())
    }

    fn recv_msg_impl(&self, buf: &mut MsgBuf<'_>, discard: bool) -> io::Result<RecvResult> {
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
                        debug_assert!(!buf.unfilled_part().is_empty());
                        continue;
                    }
                    Err(e) => {
                        if more_data && discard {
                            // A partially successful partial read must result in the rest of the
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
                    if partial && discard {
                        // This is irrelevant to normal operation of downstream
                        // programs, but still makes them easier to debug.
                        let _ = self.discard_msg();
                    }
                    return Err(e);
                }
            };
            unsafe {
                // SAFETY: this one is on Windows
                buf.advance_init_and_set_fill(buf.len_filled() + incr)
            };
        }
        buf.has_msg = true;
        Ok(if spilled { RecvResult::Spilled } else { RecvResult::Fit })
    }

    #[inline]
    fn recv_msg(&self, buf: &mut MsgBuf<'_>) -> io::Result<RecvResult> {
        self.recv_msg_impl(buf, true)
    }
}

impl<Sm: PipeModeTag> RecvMsg for &PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(&mut self, buf: &mut MsgBuf<'_>, _: Option<&mut NoAddrBuf>) -> io::Result<RecvResult> {
        self.raw.recv_msg(buf)
    }
}
impl<Sm: PipeModeTag> RecvMsg for PipeStream<pipe_mode::Messages, Sm> {
    type Error = io::Error;
    type AddrBuf = NoAddrBuf;
    #[inline]
    fn recv_msg(&mut self, buf: &mut MsgBuf<'_>, _: Option<&mut NoAddrBuf>) -> io::Result<RecvResult> {
        (&*self).recv_msg(buf, None)
    }
}
