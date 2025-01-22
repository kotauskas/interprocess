use {
    super::*,
    crate::os::windows::limbo::{
        tokio::{send_off, Corpse},
        LIMBO_ERR, REBURY_ERR,
    },
};

impl RawPipeStream {
    pub(super) fn inner(&self) -> &InnerTokio { self.inner.as_ref().expect(LIMBO_ERR) }
}

impl Drop for RawPipeStream {
    fn drop(&mut self) {
        let corpse = self.inner.take().map(Corpse::from).expect(REBURY_ERR);
        if self.needs_flush.get_mut() {
            send_off(corpse);
        }
    }
}
