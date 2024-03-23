use super::super::named_pipe::local_socket::tokio as np_impl;
use crate::local_socket::{
	tokio::{prelude::*, Listener, Stream},
	Name,
};
use std::io;

#[inline]
pub fn bind(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind(name).map(Listener::from)
}

#[inline]
pub fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Listener> {
	np_impl::Listener::bind_without_name_reclamation(name).map(Listener::from)
}

pub async fn connect(name: Name<'_>) -> io::Result<Stream> {
	np_impl::Stream::connect(name).await.map(Stream::from)
}
