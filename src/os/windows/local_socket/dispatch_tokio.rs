use super::super::named_pipe::local_socket::tokio as np_impl;
use crate::local_socket::{
	tokio::{prelude::*, Listener, Stream},
	ListenerOptions, Name,
};
use std::io;

#[inline]
pub fn from_options(options: ListenerOptions<'_>) -> io::Result<Listener> {
	options
		.create_tokio_as::<np_impl::Listener>()
		.map(Listener::from)
}

pub async fn connect(name: Name<'_>) -> io::Result<Stream> {
	np_impl::Stream::connect(name).await.map(Stream::from)
}
