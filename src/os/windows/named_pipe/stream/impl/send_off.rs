use super::*;
use crate::os::windows::limbo::{
	sync::{send_off, Corpse},
	LIMBO_ERR, REBURY_ERR,
};

impl RawPipeStream {
	pub(super) fn file_handle(&self) -> &FileHandle {
		self.handle.as_ref().expect(LIMBO_ERR)
	}
}

impl Drop for RawPipeStream {
	fn drop(&mut self) {
		let corpse = Corpse {
			handle: self.handle.take().expect(REBURY_ERR),
			is_server: self.is_server,
		};
		if self.needs_flush.get_mut() {
			send_off(corpse);
		}
	}
}
