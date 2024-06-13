use crate::{
	os::windows::{
		decode_eof,
		named_pipe::{PipeMode, WaitTimeout},
		winprelude::*,
		FileHandle,
	},
	AsMutPtr, HandleOrErrno, OrErrno, RawOsErrorExt, SubUsizeExt,
};
use std::{io, mem::MaybeUninit, os::windows::prelude::*, ptr};
use widestring::U16CStr;
use windows_sys::Win32::{
	Foundation::{ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE},
	Storage::FileSystem::{
		CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
		FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
	},
	System::Pipes::{
		GetNamedPipeHandleStateW, GetNamedPipeInfo, PeekNamedPipe, SetNamedPipeHandleState,
		WaitNamedPipeW, PIPE_NOWAIT,
	},
};

fn optional_out_ptr<T>(outref: Option<&mut T>) -> *mut T {
	match outref {
		Some(outref) => outref.as_mut_ptr(),
		None => ptr::null_mut(),
	}
}

/// Helper for several functions that take a handle and a u32 out-pointer.
pub(crate) unsafe fn hget(
	handle: BorrowedHandle<'_>,
	f: unsafe extern "system" fn(HANDLE, *mut u32) -> i32,
) -> io::Result<u32> {
	let mut x: u32 = 0;
	unsafe { f(handle.as_int_handle(), x.as_mut_ptr()) }.true_val_or_errno(x)
}

pub(crate) fn get_np_info(
	handle: BorrowedHandle<'_>,
	flags: Option<&mut u32>,
	in_buf: Option<&mut u32>,
	out_buf: Option<&mut u32>,
	max_instances: Option<&mut u32>,
) -> io::Result<()> {
	unsafe {
		GetNamedPipeInfo(
			handle.as_int_handle(),
			optional_out_ptr(flags),
			optional_out_ptr(in_buf),
			optional_out_ptr(out_buf),
			optional_out_ptr(max_instances),
		)
	}
	.true_val_or_errno(())
}

pub(crate) fn get_np_handle_state(
	handle: BorrowedHandle<'_>,
	mode: Option<&mut u32>,
	cur_instances: Option<&mut u32>,
	max_collection_count: Option<&mut u32>,
	collect_data_timeout: Option<&mut u32>,
	mut username: Option<&mut [MaybeUninit<u16>]>,
) -> io::Result<()> {
	// TODO(2.3.0) expose the rest of the owl as public API
	unsafe {
		GetNamedPipeHandleStateW(
			handle.as_int_handle(),
			optional_out_ptr(mode),
			optional_out_ptr(cur_instances),
			optional_out_ptr(max_collection_count),
			optional_out_ptr(collect_data_timeout),
			username
				.as_deref_mut()
				.map(|s| s.as_mut_ptr().cast())
				.unwrap_or(ptr::null_mut()),
			username
				.map(|s| u32::try_from(s.len()).unwrap_or(u32::MAX))
				.unwrap_or(0),
		)
	}
	.true_val_or_errno(())
}

pub(crate) fn set_np_handle_state(
	handle: BorrowedHandle<'_>,
	mode: Option<u32>,
	max_collection_count: Option<u32>,
	collect_data_timeout: Option<u32>,
) -> io::Result<()> {
	let (mut mode_, has_mode) = (mode.unwrap_or_default(), mode.is_some());
	let (mut mcc, has_mcc) = (
		max_collection_count.unwrap_or_default(),
		max_collection_count.is_some(),
	);
	let (mut cdt, has_cdt) = (
		collect_data_timeout.unwrap_or_default(),
		collect_data_timeout.is_some(),
	);
	let null = ptr::null_mut();
	unsafe {
		SetNamedPipeHandleState(
			handle.as_int_handle(),
			if has_mode { mode_.as_mut_ptr() } else { null },
			if has_mcc { mcc.as_mut_ptr() } else { null },
			if has_cdt { cdt.as_mut_ptr() } else { null },
		)
	}
	.true_val_or_errno(())
}

#[inline]
pub(crate) fn get_flags(handle: BorrowedHandle<'_>) -> io::Result<u32> {
	let mut flags: u32 = 0;
	get_np_info(handle, Some(&mut flags), None, None, None)?;
	Ok(flags)
}

#[allow(dead_code)]
pub(crate) fn get_np_handle_mode(handle: BorrowedHandle<'_>) -> io::Result<u32> {
	let mut mode = 0_u32;
	get_np_handle_state(handle, Some(&mut mode), None, None, None, None)?;
	Ok(mode)
}

pub(crate) fn peek_msg_len(handle: BorrowedHandle<'_>) -> io::Result<usize> {
	let mut msglen: u32 = 0;
	let rslt = unsafe {
		PeekNamedPipe(
			handle.as_int_handle(),
			ptr::null_mut(),
			0,
			ptr::null_mut(),
			ptr::null_mut(),
			msglen.as_mut_ptr(),
		)
	}
	.true_val_or_errno(msglen.to_usize());
	decode_eof(rslt)
}

fn modes_to_access_flags(recv: Option<PipeMode>, send: Option<PipeMode>) -> u32 {
	let mut access_flags = 0;
	if recv.is_some() {
		access_flags |= GENERIC_READ;
	}
	if recv == Some(PipeMode::Messages) {
		access_flags |= FILE_WRITE_ATTRIBUTES;
	}
	if send.is_some() {
		access_flags |= GENERIC_WRITE;
	}
	access_flags
}

pub(crate) fn connect_without_waiting(
	path: &U16CStr,
	recv: Option<PipeMode>,
	send: Option<PipeMode>,
	overlapped: bool,
) -> io::Result<FileHandle> {
	let access_flags = modes_to_access_flags(recv, send);
	let flags = if overlapped { FILE_FLAG_OVERLAPPED } else { 0 };
	match unsafe {
		CreateFileW(
			path.as_ptr(),
			access_flags,
			FILE_SHARE_READ | FILE_SHARE_WRITE,
			ptr::null_mut(),
			OPEN_EXISTING,
			flags,
			0,
		)
		.handle_or_errno()
		.map(|h|
			// SAFETY: we just created this handle
			FileHandle::from(OwnedHandle::from_raw_handle(h.to_std())))
	} {
		Err(e) if e.raw_os_error().eeq(ERROR_PIPE_BUSY) => Err(io::ErrorKind::WouldBlock.into()),
		els => els,
	}
}

pub(crate) fn set_nonblocking_given_readmode(
	handle: BorrowedHandle<'_>,
	nonblocking: bool,
	recv: Option<PipeMode>,
) -> io::Result<()> {
	// PIPE_READMODE_BYTE is the default
	let mut mode = recv.unwrap_or(PipeMode::Bytes).to_readmode();
	if nonblocking {
		mode |= PIPE_NOWAIT;
	}
	set_np_handle_state(handle, Some(mode), None, None)
}

pub(crate) fn block_for_server(path: &U16CStr, timeout: WaitTimeout) -> io::Result<()> {
	unsafe { WaitNamedPipeW(path.as_ptr().cast_mut(), timeout.to_raw()) }.true_val_or_errno(())
}
