use crate::{
    os::windows::{
        named_pipe::{
            enums::{PipeMode, PipeStreamRole},
            pipe_mode,
            tokio::{PipeStream, RawPipeStream},
            PipeListenerOptions, PipeModeTag,
        },
        winprelude::*,
    },
    Sealed,
};
use std::{
    fmt::{self, Debug, Formatter},
    io,
    marker::PhantomData,
    mem::replace,
};
use tokio::{net::windows::named_pipe::NamedPipeServer as TokioNPServer, sync::Mutex};

/// A Tokio-based async server for a named pipe, asynchronously listening for connections to clients
/// and producing asynchronous pipe streams.
///
/// Note that this type does not correspond to any Tokio object, and is an invention of Interprocess
/// in its entirety.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation
/// for more.
///
/// # Examples
///
/// ## Basic server
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use tokio::{io::{AsyncReadExt, AsyncWriteExt}, try_join};
/// use interprocess::os::windows::named_pipe::{pipe_mode, tokio::*, PipeListenerOptions};
/// use std::{path::Path, io};
///
/// // Describe the things we do when we've got a connection ready.
/// async fn handle_conn(conn: DuplexPipeStream<pipe_mode::Bytes>) -> io::Result<()> {
///     // Split the connection into two halves to process
///     // received and sent data concurrently.
///     let (mut recver, mut sender) = conn.split();
///
///     // Allocate a sizeable buffer for receiving.
///     // This size should be enough and should be easy to find for the allocator.
///     let mut buffer = String::with_capacity(128);
///
///     // Describe the send operation as first sending our whole message, and
///     // then shutting down the send half to send an EOF to help the other
///     // side determine the end of the transmission.
///     let send = async {
///         sender.write_all(b"Hello from server!").await?;
///         sender.shutdown().await?;
///         Ok(())
///     };
///
///     // Describe the receive operation as receiving into our big buffer.
///     let recv = recver.read_to_string(&mut buffer);
///
///     // Run both the send-and-invoke-EOF operation and the receive operation concurrently.
///     try_join!(recv, send)?;
///
///     // Dispose of our connection right now and not a moment later because I want to!
///     drop((recver, sender));
///
///     // Produce our output!
///     println!("Client answered: {}", buffer.trim());
///     Ok(())
/// }
///
/// static PIPE_NAME: &str = "Example";
///
/// // Create our listener.
/// let listener = PipeListenerOptions::new()
///     .path(Path::new(PIPE_NAME))
///     .create_tokio_duplex::<pipe_mode::Bytes>()?;
///
/// // The syncronization between the server and client, if any is used, goes here.
/// eprintln!(r"Server running at \\.\pipe\{PIPE_NAME}");
///
/// // Set up our loop boilerplate that processes our incoming connections.
/// loop {
///     // Sort out situations when establishing an incoming connection caused an error.
///     let conn = match listener.accept().await {
///         Ok(c) => c,
///         Err(e) => {
///             eprintln!("There was an error with an incoming connection: {e}");
///             continue;
///         }
///     };
///
///     // Spawn new parallel asynchronous tasks onto the Tokio runtime
///     // and hand the connection over to them so that multiple clients
///     // could be processed simultaneously in a lightweight fashion.
///     tokio::spawn(async move {
///         // The outer match processes errors that happen when we're
///         // connecting to something. The inner if-let processes errors that
///         // happen during the connection.
///         if let Err(e) = handle_conn(conn).await {
///             eprintln!("error while handling connection: {e}");
///         }
///     });
/// }
/// # Ok(()) }
/// ```
pub struct PipeListener<Rm: PipeModeTag, Sm: PipeModeTag> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    stored_instance: Mutex<TokioNPServer>,
    _phantom: PhantomData<(Rm, Sm)>,
}
impl<Rm: PipeModeTag, Sm: PipeModeTag> PipeListener<Rm, Sm> {
    const STREAM_ROLE: PipeStreamRole = PipeStreamRole::get_for_rm_sm::<Rm, Sm>();

    /// Asynchronously waits until a client connects to the named pipe, creating a `Stream` to
    /// communicate with the pipe.
    pub async fn accept(&self) -> io::Result<PipeStream<Rm, Sm>> {
        let instance_to_hand_out = {
            let mut stored_instance = self.stored_instance.lock().await;
            stored_instance.connect().await?;
            let new_instance = self.create_instance()?;
            replace(&mut *stored_instance, new_instance)
        };

        let raw = RawPipeStream::new_server(instance_to_hand_out);
        Ok(PipeStream::new(raw))
    }

    fn create_instance(&self) -> io::Result<TokioNPServer> {
        self.config
            .create_instance(false, false, true, Self::STREAM_ROLE, Rm::MODE)
            .and_then(npserver_from_handle)
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
    /// Creates a Tokio pipe listener from the builder. See the
    /// [non-async `create` method on `PipeListenerOptions`](PipeListenerOptions::create) for more.
    ///
    /// The `nonblocking` parameter is ignored and forced to be enabled.
    fn create_tokio<Rm: PipeModeTag, Sm: PipeModeTag>(&self) -> io::Result<PipeListener<Rm, Sm>>;
    /// Alias for [`.create_tokio()`](PipeListenerOptionsExt::create_tokio) with the same `Rm` and
    /// `Sm`.
    #[inline]
    fn create_tokio_duplex<M: PipeModeTag>(&self) -> io::Result<PipeListener<M, M>> {
        self.create_tokio::<M, M>()
    }
    /// Alias for [`.create_tokio()`](PipeListenerOptionsExt::create_tokio) with an `Sm` of
    /// [`pipe_mode::None`].
    #[inline]
    fn create_tokio_recv_only<Rm: PipeModeTag>(
        &self,
    ) -> io::Result<PipeListener<Rm, pipe_mode::None>> {
        self.create_tokio::<Rm, pipe_mode::None>()
    }
    /// Alias for [`.create_tokio()`](PipeListenerOptionsExt::create_tokio) with an `Rm` of
    /// [`pipe_mode::None`].
    #[inline]
    fn create_tokio_send_only<Sm: PipeModeTag>(
        &self,
    ) -> io::Result<PipeListener<pipe_mode::None, Sm>> {
        self.create_tokio::<pipe_mode::None, Sm>()
    }
}
impl PipeListenerOptionsExt for PipeListenerOptions<'_> {
    fn create_tokio<Rm: PipeModeTag, Sm: PipeModeTag>(&self) -> io::Result<PipeListener<Rm, Sm>> {
        let (owned_config, instance) =
            _create_tokio(self, PipeListener::<Rm, Sm>::STREAM_ROLE, Rm::MODE)?;
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
    recv_mode: Option<PipeMode>,
) -> io::Result<(PipeListenerOptions<'static>, TokioNPServer)> {
    // Shadow to avoid mixing them up.
    let mut config = config.to_owned();

    // Tokio should ideally already set that, but let's do it just in case.
    config.nonblocking = false;

    let instance = config
        .create_instance(true, config.nonblocking, true, role, recv_mode)
        .and_then(npserver_from_handle)?;

    Ok((config, instance))
}

fn npserver_from_handle(handle: OwnedHandle) -> io::Result<TokioNPServer> {
    unsafe { TokioNPServer::from_raw_handle(handle.into_raw_handle()) }
}
