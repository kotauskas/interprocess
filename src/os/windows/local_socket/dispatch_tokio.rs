use super::super::named_pipe::local_socket::tokio as np_impl;
use crate::local_socket::{
	tokio::{Listener, Stream},
	Name,
};
use std::io;

#[inline]
pub fn bind(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind(name, true).map(Listener::from)
}

#[inline]
pub fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind(name, false).map(Listener::from)
}

pub async fn connect(name: Name<'_>) -> io::Result<Stream> {
	np_impl::Stream::connect(name).await.map(Stream::from)
}
