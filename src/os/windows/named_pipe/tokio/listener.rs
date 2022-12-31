use crate::{
    os::windows::{
        imports::*,
        named_pipe::{
            enums::{PipeMode, PipeStreamRole},
            tokio::{PipeStream, RawPipeStream},
            PipeListenerOptions, PipeModeTag,
        },
    },
    Sealed,
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    marker::PhantomData,
    mem::replace,
};
use tokio::sync::Mutex;

/// A Tokio-based async server for a named pipe, asynchronously listening for connections to clients and producing asynchronous pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
pub struct PipeListener<Rm: PipeModeTag, Sm: PipeModeTag> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    stored_instance: Mutex<TokioNPServer>,
    _phantom: PhantomData<(Rm, Sm)>,
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeListener<Rm, Sm> {
    const STREAM_ROLE: PipeStreamRole = PipeStreamRole::get_for_rm_sm::<Rm, Sm>();

    /// Asynchronously waits until a client connects to the named pipe, creating a `Stream` to communicate with the pipe.
    pub async fn accept(&self) -> io::Result<PipeStream<Rm, Sm>> {
        let instance_to_hand_out = {
            let mut stored_instance = self.stored_instance.lock().await;
            stored_instance.connect().await?;
            let new_instance = self.create_instance()?;
            replace(&mut *stored_instance, new_instance)
        };

        let raw = RawPipeStream::Server(instance_to_hand_out);
        Ok(PipeStream::new(raw))
    }

    fn create_instance(&self) -> io::Result<TokioNPServer> {
        let handle = self
            .config
            .create_instance(false, false, true, Self::STREAM_ROLE, Rm::MODE)?;
        // SAFETY: we just created this handle
        Ok(unsafe { TokioNPServer::from_raw_handle(handle)? })
    }
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> Debug for PipeListener<Rm, Sm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeListener")
            .field("config", &self.config)
            .field("instance", &self.stored_instance)
            .finish()
    }
}

/// Extends [`PipeListenerOptions`] with a constructor method for the Tokio [`PipeListener`].
pub trait PipeListenerOptionsExt: Sealed {
    /// Creates a Tokio pipe listener from the builder. See the [non-async `create` method on `PipeListenerOptions`](PipeListenerOptions::create) for more.
    ///
    /// The `nonblocking` parameter is ignored and forced to be enabled.
    fn create_tokio<Rm: PipeModeTag, Sm: PipeModeTag>(&self) -> io::Result<PipeListener<Rm, Sm>>;
}
impl PipeListenerOptionsExt for PipeListenerOptions<'_> {
    fn create_tokio<Rm: PipeModeTag, Sm: PipeModeTag>(&self) -> io::Result<PipeListener<Rm, Sm>> {
        let (owned_config, instance) = _create_tokio(self, PipeListener::<Rm, Sm>::STREAM_ROLE, Rm::MODE)?;
        Ok(PipeListener {
            config: owned_config,
            stored_instance: Mutex::new(instance),
            _phantom: PhantomData,
        })
    }
}
impl Sealed for PipeListenerOptions<'_> {}
fn _create_tokio(
    config: &PipeListenerOptions<'_>,
    role: PipeStreamRole,
    read_mode: Option<PipeMode>,
) -> io::Result<(PipeListenerOptions<'static>, TokioNPServer)> {
    // Shadow to avoid mixing them up.
    let mut config = config.to_owned();

    // Tokio should ideally already set that, but let's do it just in case.
    config.nonblocking = false;

    let instance = {
        let handle = config.create_instance(true, config.nonblocking, true, role, read_mode)?;
        unsafe {
            // SAFETY: we just created this handle, so we know it's unique (and we've checked
            // that it's valid)
            TokioNPServer::from_raw_handle(handle)?
        }
    };

    Ok((config, instance))
}
