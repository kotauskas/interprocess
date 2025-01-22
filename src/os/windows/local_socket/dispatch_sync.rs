use {
    super::super::named_pipe::local_socket as np_impl,
    crate::local_socket::{prelude::*, Listener, ListenerOptions, Name, Stream},
    std::io,
};

#[inline]
pub fn from_options(options: ListenerOptions<'_>) -> io::Result<Listener> {
    options.create_sync_as::<np_impl::Listener>().map(Listener::from)
}

pub fn connect(name: Name<'_>) -> io::Result<Stream> {
    np_impl::Stream::connect(name).map(Stream::from)
}
