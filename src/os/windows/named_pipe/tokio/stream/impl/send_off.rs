use super::{
    super::limbo::{send_off, Corpse},
    *,
};
use crate::os::windows::named_pipe::{LIMBO_ERR, REBURY_ERR};

impl RawPipeStream {
    pub(super) fn inner(&self) -> &InnerTokio {
        self.inner.as_ref().expect(LIMBO_ERR)
    }
}

impl Drop for RawPipeStream {
    fn drop(&mut self) {
        let corpse = self.inner.take().map(Corpse).expect(REBURY_ERR);
        if self.needs_flush.get() {
            send_off(corpse);
        }
    }
}
