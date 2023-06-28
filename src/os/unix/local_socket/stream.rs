use {
    super::local_socket_name_to_ud_socket_path,
    crate::{
        local_socket::ToLocalSocketName,
        os::unix::udsocket::{UdSocket, UdStream},
    },
    std::{
        fmt::{self, Debug, Formatter},
        io::{self, prelude::*, IoSlice, IoSliceMut},
        os::unix::io::AsRawFd,
    },
};

pub struct LocalSocketStream(pub(super) UdStream);
impl LocalSocketStream {
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStream::connect(path)?;
        Ok(Self(inner))
    }
    pub fn peer_pid(&self) -> io::Result<u32> {
        #[cfg(uds_ucred)]
        {
            self.0.get_peer_credentials().map(|ucred| ucred.pid as u32)
        }
        #[cfg(not(uds_ucred))]
        {
            Err(io::Error::new(io::ErrorKind::Other, "not supported"))
        }
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}
impl Read for LocalSocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
}
impl Write for LocalSocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl Debug for LocalSocketStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("fd", &self.0.as_raw_fd())
            .finish()
    }
}
forward_handle!(unix: LocalSocketStream);
