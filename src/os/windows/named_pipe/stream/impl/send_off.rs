use super::*;
use crate::os::windows::named_pipe::stream::limbo::{send_off, Corpse};

pub(crate) static LIMBO_ERR: &str =
	"attempt to perform operation on pipe stream which has been sent off to limbo";
pub(crate) static REBURY_ERR: &str = "attempt to bury same pipe stream twice";

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
		if self.needs_flush.get() {
			send_off(corpse);
		}
	}
}
