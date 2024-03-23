use super::super::uds_local_socket::tokio as uds_impl;
use crate::local_socket::{
	tokio::{prelude::*, Listener, Stream},
	Name,
};
use std::io;

#[inline]
pub fn bind(name: Name<'_>) -> io::Result<Listener> {
	uds_impl::Listener::bind(name).map(Listener::from)
}

#[inline]
pub fn bind_without_name_reclamation(name: Name<'_>) -> io::Result<Listener> {
	uds_impl::Listener::bind_without_name_reclamation(name).map(Listener::from)
}

pub async fn connect(name: Name<'_>) -> io::Result<Stream> {
	uds_impl::Stream::connect(name).await.map(Stream::from)
}
