//! Methods and trait implementations for `PipeStream`.

macro_rules! same_clsrv {
	($nm:ident in $var:expr => $e:expr) => {
		match $var {
			InnerTokio::Server($nm) => $e,
			InnerTokio::Client($nm) => $e,
		}
	};
}

mod ctor;
mod debug;
mod handle;
mod recv_bytes;
mod send;
mod send_off;

use super::*;
use crate::os::windows::{
	named_pipe::{
		c_wrappers::{self, hget},
		PipeMode,
	},
	winprelude::*,
};
use std::{
	future::Future,
	pin::Pin,
	task::{ready, Context, Poll},
};
use tokio::net::windows::named_pipe::{
	NamedPipeClient as TokioNPClient, NamedPipeServer as TokioNPServer,
};
use windows_sys::Win32::System::Pipes;

impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeStream<Rm, Sm> {
	/// Splits the pipe stream by value, returning a receive half and a send half. The stream is
	/// closed when both are dropped, kind of like an `Arc` (which is how it's implemented under the
	/// hood).
	pub fn split(mut self) -> (RecvPipeStream<Rm>, SendPipeStream<Sm>) {
		let (raw_ac, raw_a) = (self.raw.refclone(), self.raw);
		(
			RecvPipeStream {
				raw: raw_a,
				flusher: (),
				_phantom: PhantomData,
			},
			SendPipeStream {
				raw: raw_ac,
				flusher: self.flusher,
				_phantom: PhantomData,
			},
		)
	}
	/// Attempts to reunite a receive half with a send half to yield the original stream back,
	/// returning both halves as an error if they belong to different streams (or when using
	/// this method on streams that were never split to begin with).
	pub fn reunite(rh: RecvPipeStream<Rm>, sh: SendPipeStream<Sm>) -> ReuniteResult<Rm, Sm> {
		if !MaybeArc::ptr_eq(&rh.raw, &sh.raw) {
			return Err(ReuniteError { rh, sh });
		}
		let PipeStream {
			mut raw, flusher, ..
		} = sh;
		drop(rh);
		raw.try_make_owned();
		Ok(PipeStream {
			raw,
			flusher,
			_phantom: PhantomData,
		})
	}
	/// Retrieves the process identifier of the client side of the named pipe connection.
	#[inline]
	pub fn client_process_id(&self) -> io::Result<u32> {
		unsafe { hget(self.as_handle(), Pipes::GetNamedPipeClientProcessId) }
	}
	/// Retrieves the session identifier of the client side of the named pipe connection.
	#[inline]
	pub fn client_session_id(&self) -> io::Result<u32> {
		unsafe { hget(self.as_handle(), Pipes::GetNamedPipeClientSessionId) }
	}
	/// Retrieves the process identifier of the server side of the named pipe connection.
	#[inline]
	pub fn server_process_id(&self) -> io::Result<u32> {
		unsafe { hget(self.as_handle(), Pipes::GetNamedPipeServerProcessId) }
	}
	/// Retrieves the session identifier of the server side of the named pipe connection.
	#[inline]
	pub fn server_session_id(&self) -> io::Result<u32> {
		unsafe { hget(self.as_handle(), Pipes::GetNamedPipeServerSessionId) }
	}
	/// Returns `true` if the stream was created by a listener (server-side), `false` if it was
	/// created by connecting to a server (server-side).
	#[inline]
	pub fn is_server(&self) -> bool {
		matches!(self.raw.inner(), &InnerTokio::Server(..))
	}
	/// Returns `true` if the stream was created by connecting to a server (client-side), `false` if
	/// it was created by a listener (server-side).
	#[inline]
	pub fn is_client(&self) -> bool {
		!self.is_server()
	}
}
