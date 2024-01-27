use super::*;

impl RawPipeStream {
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let r = self.file_handle().write(buf);
        if r.is_ok() {
            self.needs_flush.mark_dirty();
        }
        r
    }

    fn flush(&self) -> io::Result<()> {
        if self.needs_flush.on_flush() {
            let r = self.file_handle().flush();
            if r.is_err() {
                self.needs_flush.mark_dirty();
            }
            r
        } else {
            Ok(())
        }
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag + PmtNotNone> PipeStream<Rm, Sm> {
    /// Flushes the stream, blocking until the send buffer is empty (has been received by the other
    /// end in its entirety).
    ///
    /// Only available on streams that have a send mode.
    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        self.raw.flush()
    }
    /// Marks the stream as unflushed, preventing elision of the next flush operation (which
    /// includes limbo).
    #[inline]
    pub fn mark_dirty(&self) {
        self.raw.needs_flush.mark_dirty();
    }
    /// Assumes that the other side has consumed everything that's been written so far. This will
    /// turn the next flush into a no-op, but will cause the send buffer to be cleared when the
    /// stream is closed, since it won't be sent to limbo.
    #[inline]
    pub fn assume_flushed(&self) {
        self.raw.needs_flush.on_flush();
    }
    /// Drops the stream without sending it to limbo. This is the same as calling
    /// `assume_flushed()` right before dropping it.
    #[inline]
    pub fn evade_limbo(self) {
        self.assume_flushed();
    }
}

impl<Rm: PipeModeTag> PipeStream<Rm, pipe_mode::Messages> {
    /// Sends a message into the pipe, returning how many bytes were successfully sent (typically
    /// equal to the size of what was requested to be sent).
    #[inline]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.raw.send(buf)
    }
}

impl<Rm: PipeModeTag> Write for &PipeStream<Rm, pipe_mode::Bytes> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.raw.send(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.raw.flush()
    }
}
impl<Rm: PipeModeTag> Write for PipeStream<Rm, pipe_mode::Bytes> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self as &PipeStream<_, _>).write(buf)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        (self as &PipeStream<_, _>).flush()
    }
}
