//! Windows-specific functionality for Tokio-based unnamed pipes.

use super::CreationOptions;
use crate::{
	os::windows::winprelude::*,
	unnamed_pipe::{
		tokio::{Recver as PubRecver, Sender as PubSender},
		Recver as SyncRecver, Sender as SyncSender,
	},
	Sealed,
};
use std::io;
use tokio::fs::File;

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
impl TryFrom<SyncSender> for Sender {
	type Error = io::Error;
	fn try_from(tx: SyncSender) -> io::Result<Self> {
		Ok(Self(File::from_std(
			<std::fs::File as From<OwnedHandle>>::from(tx.into()),
		)))
	}
}
multimacro! {
	Sender,
	pinproj_for_unpin(File),
	forward_rbv(File, &),
	forward_tokio_write,
	forward_as_handle,
}

// TODO do something about flushing
