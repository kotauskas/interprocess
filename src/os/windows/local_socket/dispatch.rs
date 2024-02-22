use super::super::named_pipe::local_socket as np_impl;
use crate::local_socket::{prelude::*, Listener, Name, Stream};
use std::io;

#[inline]
pub fn bind(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind(name).map(Listener::from)
}

#[inline]
pub fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind(name).map(Listener::from)
}

pub fn connect(name: Name<'_>) -> io::Result<Stream> {
	np_impl::Stream::connect(name).map(Stream::from)
}
