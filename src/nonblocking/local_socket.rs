use blocking::{unblock, Unblock};
use futures::{
    stream::{unfold, Stream},
    AsyncRead, AsyncWrite,
};
use std::{
    io,
    sync::Arc,
};

use crate::local_socket::{self as sync, ToLocalSocketName};

#[derive(Clone)]
pub struct LocalSocketListener {
    inner: Arc<sync::LocalSocketListener>,
}

impl LocalSocketListener {
    pub async fn bind<'a, T>(name: T) -> io::Result<Self>
    where
        T: ToLocalSocketName<'a> + Send + 'static,
    {
        Ok(Self {
            inner: Arc::new(unblock(move || sync::LocalSocketListener::bind(name)).await?),
        })
    }
    pub async fn accept(&self) -> io::Result<LocalSocketStream> {
        let s = self.inner.clone();
        Ok(LocalSocketStream {
            inner: Unblock::new(unblock(move || s.accept()).await?),
        })
    }
    pub fn incoming(&self) -> impl Stream<Item = std::io::Result<LocalSocketStream>> {
        // TODO (?) : effectively `clone`s the Arc twice for every Item.
        //      This could be fixed by copying the code from the `accept` function
        //      or creating an alernative `accept` function that takes `self`
        let s = self.clone();
        unfold((), move |()| {
            let s = s.inner.clone();
            async move {
                Some((
                    unblock(move || s.accept())
                        .await
                        .map(|x| LocalSocketStream {
                            inner: Unblock::new(x),
                        }),
                    (),
                ))
            }
        })
    }
}

pub struct LocalSocketStream {
    inner: Unblock<sync::LocalSocketStream>,
}
impl LocalSocketStream {
    pub async fn connect<'a, T>(name: T) -> io::Result<Self>
    where
        T: ToLocalSocketName<'a> + Send + 'static,
    {
        Ok(Self {
            inner: Unblock::new(unblock(move || sync::LocalSocketStream::connect(name)).await?),
        })
    }
}

use futures::task::{Context, Poll};
use std::pin::Pin;
impl AsyncRead for LocalSocketStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        AsyncRead::poll_read(Pin::new(&mut self.inner), cx, buf)
    }
}
impl AsyncWrite for LocalSocketStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        AsyncWrite::poll_write(Pin::new(&mut self.inner), cx, buf)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), futures::io::Error>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.inner), cx)
    }
    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), futures::io::Error>> {
        AsyncWrite::poll_close(Pin::new(&mut self.inner), cx)
    }
}
