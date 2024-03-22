//! Trait bound utilities.

use std::io::prelude::*;
#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite};

// TODO macro..?

/// [`Read`] by reference.
pub trait RefRead {
	#[doc(hidden)]
	#[allow(private_bounds)]
	type Read<'a>: Read + Is<&'a Self>
	where
		Self: 'a;
	/// Returns `self` with the guarantee that `&Self` implements `Read` encoded in a way which is
	/// visible to Rust's type system.
	fn as_read(&self) -> Self::Read<'_>;
}
impl<T: ?Sized> RefRead for T
where
	for<'a> &'a T: Read,
{
	type Read<'a> = &'a Self
	where Self: 'a;
	#[inline(always)]
	fn as_read(&self) -> Self::Read<'_> {
		self
	}
}
/// [`Write`] by reference.
pub trait RefWrite {
	#[doc(hidden)]
	#[allow(private_bounds)]
	type Write<'a>: Write + Is<&'a Self>
	where
		Self: 'a;
	/// Returns `self` with the guarantee that `&Self` implements `Write` encoded in a way which is
	/// visible to Rust's type system.
	fn as_write(&self) -> Self::Write<'_>;
}
impl<T: ?Sized> RefWrite for T
where
	for<'a> &'a T: Write,
{
	type Write<'a> = &'a Self
	where Self: 'a;
	#[inline(always)]
	fn as_write(&self) -> Self::Write<'_> {
		self
	}
}

/// [Tokio's AsyncRead](TokioAsyncRead) by reference.
#[cfg(feature = "tokio")]
pub trait RefTokioAsyncRead {
	#[doc(hidden)]
	#[allow(private_bounds)]
	type Read<'a>: TokioAsyncRead + Is<&'a Self>
	where
		Self: 'a;
	/// Returns `self` with the guarantee that `&Self` implements `TokioAsyncRead` encoded in a way
	/// which is visible to Rust's type system.
	fn as_tokio_async_read(&self) -> Self::Read<'_>;
}
#[cfg(feature = "tokio")]
impl<T: ?Sized> RefTokioAsyncRead for T
where
	for<'a> &'a T: TokioAsyncRead,
{
	type Read<'a> = &'a Self
	where Self: 'a;
	#[inline(always)]
	fn as_tokio_async_read(&self) -> Self::Read<'_> {
		self
	}
}
/// [Tokio's AsyncWrite](TokioAsyncWrite) by reference.
#[cfg(feature = "tokio")]
pub trait RefTokioAsyncWrite {
	#[doc(hidden)]
	#[allow(private_bounds)]
	type Write<'a>: TokioAsyncWrite + Is<&'a Self>
	where
		Self: 'a;
	/// Returns `self` with the guarantee that `&Self` implements `Write` encoded in a way which is
	/// visible to Rust's type system.
	fn as_tokio_async_write(&self) -> Self::Write<'_>;
}
#[cfg(feature = "tokio")]
impl<T: ?Sized> RefTokioAsyncWrite for T
where
	for<'a> &'a T: TokioAsyncWrite,
{
	type Write<'a> = &'a Self
	where Self: 'a;
	#[inline(always)]
	fn as_tokio_async_write(&self) -> Self::Write<'_> {
		self
	}
}

pub(crate) trait Is<T: ?Sized> {}
impl<T: ?Sized> Is<T> for T {}
