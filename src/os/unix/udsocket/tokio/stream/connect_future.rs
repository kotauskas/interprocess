use {
    crate::os::unix::udsocket::{UdSocketPath, UdStream as SyncUdStream},
    std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    },
};

pub struct ConnectFuture<'a, 'b> {
    pub path: &'b UdSocketPath<'a>,
}
impl Future for ConnectFuture<'_, '_> {
    type Output = io::Result<SyncUdStream>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let path = self.get_mut().path;
        match SyncUdStream::connect_nonblocking(path) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            els => Poll::Ready(els),
        }
    }
}
