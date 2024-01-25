use super::*;
use std::fmt::{self, Debug, DebugStruct, Formatter};

impl RawPipeStream {
    fn fill_fields<'a, 'b, 'c>(
        &self,
        dbst: &'a mut DebugStruct<'b, 'c>,
        readmode: Option<PipeMode>,
        writemode: Option<PipeMode>,
    ) -> &'a mut DebugStruct<'b, 'c> {
        let (tokio_object, is_server) = match self.inner() {
            InnerTokio::Server(s) => (s as _, true),
            InnerTokio::Client(c) => (c as _, false),
        };
        if let Some(readmode) = readmode {
            dbst.field("read_mode", &readmode);
        }
        if let Some(writemode) = writemode {
            dbst.field("write_mode", &writemode);
        }
        dbst.field("tokio_object", tokio_object).field("is_server", &is_server)
    }
}

impl<Rm: PipeModeTag, Sm: PipeModeTag> Debug for PipeStream<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut dbst = f.debug_struct("PipeStream");
        self.raw.fill_fields(&mut dbst, Rm::MODE, Sm::MODE);
        if Sm::MODE.is_some() {
            dbst.field("flush", &self.flush);
        }
        dbst.finish()
    }
}
