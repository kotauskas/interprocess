use crate::{
    os::windows::named_pipe::{
        enums::{PipeMode, PipeStreamRole},
        instancer::{Instance, Instancer},
        tokio::{PipeOps, TokioPipeStream},
        PipeListenerOptions, INITIAL_INSTANCER_CAPACITY,
    },
    Sealed,
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    marker::PhantomData,
    num::NonZeroU8,
    sync::RwLock,
};
use to_method::To;

/// A Tokio-based async server for a named pipe, asynchronously listening for connections to clients and producing asynchronous pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
pub struct PipeListener<Stream: TokioPipeStream> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    instancer: Instancer<PipeOps>,
    _phantom: PhantomData<fn() -> Stream>,
}
impl<Stream: TokioPipeStream> PipeListener<Stream> {
    /// Asynchronously waits until a client connects to the named pipe, creating a `Stream` to communicate with the pipe.
    pub async fn accept(&self) -> io::Result<Stream> {
        let instance = if let Some(instance) = self.instancer.allocate() {
            instance
        } else {
            self.instancer.add_instance(self.create_instance()?)
        };
        instance.instance().connect_server().await?;
        // I have no idea why, but every time I run a minimal named pipe server example without
        // this code, the second client to connect causes a "no process on the other end of the
        // pipe" error, and for some reason, performing a read or write with a zero-sized
        // buffer and discarding its result fixes this problem entirely. I'm not sure if it's a
        // crazy bug of interprocess, Tokio or even Windows, but this is the best solution I've
        // come up for.
        if Stream::READ_MODE.is_some() {
            instance.instance().dry_read().await;
        }
        if Stream::WRITE_MODE.is_some() {
            instance.instance().dry_write().await;
        }
        Ok(Stream::build(instance))
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
            .field("instances", &self.instancer)
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
        let (owned_config, instancer) = _create_tokio(self, Stream::ROLE, Stream::READ_MODE)?;
        Ok(PipeListener {
            config: owned_config,
            instancer,
            _phantom: PhantomData,
        })
    }
}
impl Sealed for PipeListenerOptions<'_> {}
fn _create_tokio(
    config: &PipeListenerOptions<'_>,
    role: PipeStreamRole,
    read_mode: Option<PipeMode>,
) -> io::Result<(PipeListenerOptions<'static>, Instancer<PipeOps>)> {
    // Shadow to avoid mixing them up.
    let mut config = config.to_owned();

    // Tokio should ideally already set that, but let's do it just in case.
    config.nonblocking = false;

    // Conversion from `Option<NonZeroU8>` to `usize`, with the `None` case corresponding to
    // the reasonable default of 8.
    let instancer_capacity = config
        .instance_limit
        .map_or(INITIAL_INSTANCER_CAPACITY, NonZeroU8::get)
        .to::<usize>();

    let first_instance = {
        let handle = config.create_instance(true, config.nonblocking, true, role, read_mode)?;
        let ops = unsafe {
            // SAFETY: we just created this handle, so we know it's unique (and we've checked
            // that it's valid)
            PipeOps::from_raw_handle(handle, true)?
        };
        Instance::create_non_taken(ops)
    };

    // Preallocate space for the instances. Actual instance creation will happen lazily.
    let mut instance_vec = Vec::with_capacity(instancer_capacity);
    // Except for the first instance, which we'll add right now.
    instance_vec.push(first_instance);

    let instancer = Instancer(RwLock::new(instance_vec));
    Ok((config, instancer))
}
