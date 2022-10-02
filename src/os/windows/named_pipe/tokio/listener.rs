use {
    crate::{
        os::windows::named_pipe::{
            enums::{PipeMode, PipeStreamRole},
            tokio::{PipeOps, TokioPipeStream},
            PipeListenerOptions,
        },
        Sealed,
    },
    std::{
        fmt::{self, Debug, Formatter},
        io,
        marker::PhantomData,
        mem::replace,
    },
    tokio::sync::Mutex,
};

/// A Tokio-based async server for a named pipe, asynchronously listening for connections to clients and producing asynchronous pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
pub struct PipeListener<Stream: TokioPipeStream> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    stored_instance: Mutex<PipeOps>,
    _phantom: PhantomData<fn() -> Stream>,
}
impl<Stream: TokioPipeStream> PipeListener<Stream> {
    /// Asynchronously waits until a client connects to the named pipe, creating a `Stream` to communicate with the pipe.
    pub async fn accept(&self) -> io::Result<Stream> {
        let instance_to_hand_out = {
            let mut stored_instance = self.stored_instance.lock().await;
            stored_instance.connect_server().await?;
            let new_instance = self.create_instance()?;
            replace(&mut *stored_instance, new_instance)
        };
        Ok(Stream::build(instance_to_hand_out.into()))
    }

    fn create_instance(&self) -> io::Result<PipeOps> {
        let handle =
            self.config
                .create_instance(false, false, true, Stream::ROLE, Stream::READ_MODE)?;
        // SAFETY: we just created this handle
        Ok(unsafe { PipeOps::from_raw_handle(handle, true)? })
    }
}
impl<Stream: TokioPipeStream> Debug for PipeListener<Stream> {
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
    fn create_tokio<Stream: TokioPipeStream>(&self) -> io::Result<PipeListener<Stream>>;
}
impl PipeListenerOptionsExt for PipeListenerOptions<'_> {
    fn create_tokio<Stream: TokioPipeStream>(&self) -> io::Result<PipeListener<Stream>> {
        let (owned_config, instance) = _create_tokio(self, Stream::ROLE, Stream::READ_MODE)?;
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
) -> io::Result<(PipeListenerOptions<'static>, PipeOps)> {
    // Shadow to avoid mixing them up.
    let mut config = config.to_owned();

    // Tokio should ideally already set that, but let's do it just in case.
    config.nonblocking = false;

    let instance = {
        let handle = config.create_instance(true, config.nonblocking, true, role, read_mode)?;
        unsafe {
            // SAFETY: we just created this handle, so we know it's unique (and we've checked
            // that it's valid)
            PipeOps::from_raw_handle(handle, true)?
        }
    };

    Ok((config, instance))
}
