//! Windows-specific functionality for Tokio-based unnamed pipes.

use crate::{
	os::windows::{unnamed_pipe::CreationOptions, winprelude::*},
	unnamed_pipe::{
		tokio::{Recver as PubRecver, Sender as PubSender},
		Recver as SyncRecver, Sender as SyncSender,
	},
	Sealed,
};
use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};
use tokio::{fs::File, io::AsyncWrite};

fn pair2pair((tx, rx): (SyncSender, SyncRecver)) -> io::Result<(PubSender, PubRecver)> {
	Ok((PubSender(tx.try_into()?), PubRecver(rx.try_into()?)))
}

#[inline]
pub(crate) fn pipe_impl() -> io::Result<(PubSender, PubRecver)> {
	pair2pair(super::pipe_impl()?)
}

/// Tokio-specific extensions to [`CreationOptions`].
#[allow(private_bounds)]
pub trait CreationOptionsExt: Sealed {
	/// Creates a Tokio-based unnamed pipe and returns its sending and receiving ends, or an error
	/// if one occurred.
	fn create_tokio(self) -> io::Result<(PubSender, PubRecver)>;
}
impl CreationOptionsExt for CreationOptions<'_> {
	#[inline]
	fn create_tokio(self) -> io::Result<(PubSender, PubRecver)> {
		pair2pair(self.create()?)
	}
}

#[derive(Debug)]
pub(crate) struct Recver(File);
impl TryFrom<SyncRecver> for Recver {
	type Error = io::Error;
	fn try_from(rx: SyncRecver) -> io::Result<Self> {
		Ok(Self(File::from_std(
			<std::fs::File as From<OwnedHandle>>::from(rx.into()),
		)))
	}
}
multimacro! {
	Recver,
	pinproj_for_unpin(File),
	forward_tokio_read,
	forward_as_handle,
}

#[derive(Debug)]
pub(crate) struct Sender(File);

impl AsyncWrite for Sender {
	#[inline]
	fn poll_write(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.0).poll_write(cx, buf)
	}
	#[inline]
	fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		// Unnamed pipes on Unix can't be flushed
		Poll::Ready(Ok(()))
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}
}

impl TryFrom<SyncSender> for Sender {
	type Error = io::Error;
	fn try_from(tx: SyncSender) -> io::Result<Self> {
		Ok(Self(File::from_std(
			<std::fs::File as From<OwnedHandle>>::from(tx.into()),
		)))
	}
}

forward_as_handle!(Sender);
