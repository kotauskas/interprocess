use crate::local_socket::{LocalSocketName, ToFsName, ToNsName};

use super::Xorshift32;
use std::{io, sync::Arc};

// TODO adjust
#[derive(Copy, Clone, Debug)]
pub struct NameGen<T: ?Sized, F: FnMut(u32) -> NameResult<T>> {
	rng: Xorshift32,
	name_fn: F,
}
impl<T: ?Sized, F: FnMut(u32) -> NameResult<T>> NameGen<T, F> {
	pub fn new(id: &'static str, name_fn: F) -> Self {
		Self {
			rng: Xorshift32::from_id(id),
			name_fn,
		}
	}
}
impl<T: ?Sized, F: FnMut(u32) -> NameResult<T>> Iterator for NameGen<T, F> {
	type Item = NameResult<T>;
	fn next(&mut self) -> Option<Self::Item> {
		Some((self.name_fn)(self.rng.next()))
	}
}

pub type NameResult<T> = io::Result<Arc<T>>;

pub fn namegen_local_socket(
	id: &'static str,
	path: bool,
) -> NameGen<LocalSocketName<'static>, impl FnMut(u32) -> io::Result<Arc<LocalSocketName<'static>>>>
{
	NameGen::new(id, move |rn| {
		if path { next_fs(rn) } else { next_ns(rn) }.map(Arc::new)
	})
}

fn next_fs(rn: u32) -> io::Result<LocalSocketName<'static>> {
	if cfg!(windows) {
		windows_path(rn)
	} else if cfg!(unix) {
		unix_path(rn)
	} else {
		unreachable!()
	}
	.to_fs_name()
}
fn next_ns(rn: u32) -> io::Result<LocalSocketName<'static>> {
	format!("@interprocess-test-{:08x}", rn).to_ns_name()
}

pub fn namegen_named_pipe(id: &'static str) -> NameGen<str, impl FnMut(u32) -> NameResult<str>> {
	NameGen::new(id, move |rn| Ok(windows_path(rn).into()))
}

fn windows_path(rn: u32) -> String {
	format!(r"\\.\pipe\interprocess-test-{rn:08x}")
}
fn unix_path(rn: u32) -> String {
	format!("/tmp/interprocess-test-{rn:08x}.sock")
}

macro_rules! make_id {
	() => {
		concat!(file!(), line!(), column!())
	};
}
