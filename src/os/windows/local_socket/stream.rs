use crate::{
    local_socket::ToLocalSocketName,
    os::windows::named_pipe::{pipe_mode, DuplexPipeStream},
};
use std::{
    io::{self, prelude::*, IoSlice, IoSliceMut},
    os::windows::prelude::*,
};

type PipeStream = DuplexPipeStream<pipe_mode::Bytes>;
#[derive(Debug)]
pub struct LocalSocketStream(pub(super) PipeStream);
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let name = name.to_local_socket_name()?;
        let inner = PipeStream::connect(name.inner())?;
        Ok(Self(inner))
    }
    #[inline]
    pub fn peer_pid(&self) -> io::Result<u32> {
        match self.0.is_server() {
            true => self.0.client_process_id(),
            false => self.0.server_process_id(),
        }
    }
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}

// The thunking already happens inside.
impl Read for LocalSocketStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
}
impl Write for LocalSocketStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
forward_as_handle!(LocalSocketStream);
forward_into_handle!(LocalSocketStream);
impl TryFrom<OwnedHandle> for LocalSocketStream {
    type Error = (OwnedHandle, io::Error);

    fn try_from(value: OwnedHandle) -> Result<Self, Self::Error> {
        todo!()
    }
}
