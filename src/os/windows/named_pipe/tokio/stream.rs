#[cfg(windows)]
use crate::os::windows::imports::ERROR_PIPE_BUSY;
use {
    crate::os::windows::named_pipe::{
        convert_path,
        tokio::{
            enums::{PipeMode, PipeStreamRole},
            imports::*,
            PipeOps, PipeStreamInternals,
        },
        PipeOps as SyncPipeOps,
    },
    std::{
        ffi::{OsStr, OsString},
        fmt::{self, Debug, Formatter},
        io,
        mem::ManuallyDrop,
        pin::Pin,
        ptr,
        task::{Context, Poll},
    },
};

mod inst {
    use {
        super::*,
        std::{
            fmt::{self, Debug, Formatter},
            ops::Deref,
            sync::{
                atomic::{AtomicBool, Ordering::*},
                Arc,
            },
        },
    };
    #[repr(transparent)]
    pub struct Instance(Arc<InstanceInner>);
    struct InstanceInner {
        ops: PipeOps,
        split: AtomicBool,
    }
    impl InstanceInner {
        pub fn new(ops: PipeOps) -> Self {
            Self {
                ops,
                split: AtomicBool::new(false),
            }
        }
    }
    impl Instance {
        pub fn new(instance: PipeOps) -> Self {
            let ii = InstanceInner::new(instance);
            Self(Arc::new(ii))
        }
        pub fn instance(&self) -> &PipeOps {
            &self.0.deref().ops
        }
        pub fn is_server(&self) -> bool {
            self.instance().is_server()
        }
        pub fn is_split(&self) -> bool {
            // This can be `Relaxed`, because the other split half is either on the same thread and thus
            // doesn't need synchronization to read the current value here, or it's on a different
            // thread and all of the relevant synchronization is performed as part of sending it to
            // another thread (same reasoning as above).
            self.0.split.load(Relaxed)
        }
        pub fn split(&self) -> Self {
            // This can be a relaxed load because a non-split instance won't ever be shared between
            // threads. From a correctness standpoint, this could even be a non-atomic load, but because
            // most architectures already guarantee well-aligned memory accesses to be atomic, there's
            // no point to writing unsafe code to do that. (Also, this condition obviously signifies
            // a bug in interprocess that can only lead to creation of excess instances at worst, so
            // there isn't a real point to making sure it never happens in release mode.)
            debug_assert!(
                !self.0.split.load(Relaxed),
                "cannot split an already split instance"
            );
            // Again, the store doesn't even need to be atomic because it won't happen concurrently.
            self.0.split.store(true, Relaxed);

            let refclone = Arc::clone(&self.0);
            Self(refclone)
        }
    }

    impl Drop for Instance {
        fn drop(&mut self) {
            self.0.split.store(false, Release);
        }
    }

    impl From<PipeOps> for Instance {
        fn from(x: PipeOps) -> Self {
            Self::new(x)
        }
    }

    impl Debug for InstanceInner {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Instance") // Not deriving to override struct name
                .field("inner", &self.ops)
                .field("split", &self.split)
                .finish()
        }
    }
    impl Debug for Instance {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(&self.0, f) // passthrough
        }
    }
}
pub(super) use inst::*;

/// Defines the properties of Tokio pipe stream types.
///
/// This is the counterpart of the [`PipeStream`](super::super::PipeStream) type for the Tokio integration.
pub trait TokioPipeStream: AsRawHandle + PipeStreamInternals {
    /// The data stream flow direction for the pipe. See the [`PipeStreamRole`] enumeration for more on what this means.
    const ROLE: PipeStreamRole;
    /// The data stream mode for the pipe. If set to `PipeMode::Bytes`, message boundaries will broken and having `READ_MODE` at `PipeMode::Messages` would be a pipe creation error.
    ///
    /// For reader streams, this value has no meaning: if the reader stream belongs to the server (client sends data, server receives), then `READ_MODE` takes the role of this value; if the reader stream belongs to the client, there is no visible difference to how the server writes data since the client specifies its read mode itself anyway.
    const WRITE_MODE: Option<PipeMode>;
    /// The data stream mode used when reading from the pipe: if `WRITE_MODE` is `PipeMode::Messages` and `READ_MODE` is `PipeMode::Bytes`, the message boundaries will be destroyed when reading even though they are retained when written. See the `PipeMode` enumeration for more on what those modes mean.
    ///
    /// For writer streams, this value has no meaning: if the writer stream belongs to the server (server sends data, client receives), then the server doesn't read data at all and thus this does not affect anything; if the writer stream belongs to the client, then the client doesn't read anything and the value is meaningless as well.
    const READ_MODE: Option<PipeMode>;
}

macro_rules! create_stream_type {
    (
        $ty:ident:
            desired_access: $desired_access:expr,
            role: $role:expr,
            read_mode: $read_mode:expr,
            write_mode: $write_mode:expr,
            extra_methods: {$($extra_methods:tt)*},
            doc: $doc:tt
    ) => {
        create_stream_type_base!(
            $ty:
            extra_methods: {
                /// Tries to connect to the specified named pipe (the `\\.\pipe\` prefix is added automatically).
                ///
                /// If there is no available server, **returns immediately** with the [`WouldBlock`](io::ErrorKind::WouldBlock) error.
                pub fn connect(name: impl AsRef<OsStr>) -> io::Result<Self> {
                    Self::_connect(name.as_ref())
                }
                fn _connect(name: &OsStr) -> io::Result<Self> {
                    let pipeops = _connect(
                        name,
                        None,
                        Self::READ_MODE.is_some(),
                        Self::WRITE_MODE.is_some(),
                    )?;
                    let instance = Instance::new(pipeops);
                    Ok(Self { instance })
                }
                /// Tries to connect to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added automatically).
                ///
                /// If there is no available server, **returns immediately** with the [`WouldBlock`](io::ErrorKind::WouldBlock) error.
                pub fn connect_to_remote(pipe_name: impl AsRef<OsStr>, hostname: impl AsRef<OsStr>) -> io::Result<Self> {
                    Self::_connect_to_remote(pipe_name.as_ref(), hostname.as_ref())
                }
                fn _connect_to_remote(pipe_name: &OsStr, hostname: &OsStr) -> io::Result<Self> {
                    let pipeops = _connect(
                        pipe_name,
                        Some(hostname),
                        Self::READ_MODE.is_some(),
                        Self::WRITE_MODE.is_some(),
                    )?;
                    let instance = Instance::new(pipeops);
                    Ok(Self { instance })
                }
                /// Returns `true` if the stream was created by a listener (server-side), `false` if it was created by connecting to a server (client-side).
                pub fn is_server(&self) -> bool {
                    matches!(self.ops(), &PipeOps::Server(_))
                }
                /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if it was created by a listener (server-side).
                pub fn is_client(&self) -> bool {
                    matches!(self.ops(), &PipeOps::Client(_))
                }
                // FIXME: cannot have into_raw_handle just yet, Tokio doesn't expose it
                /// Creates a Tokio-based async object from a given raw handle. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error (which is why the `FromRawHandle` trait can't be implemented instead).
                ///
                /// # Safety
                /// The given handle must be valid (i.e. refer to an existing kernel object) and must not be owned by any other handle container. If this is not upheld, an arbitrary handle will be closed when the returned object is dropped.
                pub unsafe fn from_raw_handle(handle: HANDLE) -> io::Result<Self> {
                    let sync_pipeops = unsafe {
                        // SAFETY: guaranteed via safety contract
                        SyncPipeOps::from_raw_handle(handle)
                    };

                    // If the wrapper type tries to read incoming data as messages, that might break
                    // if the underlying pipe has no message boundaries. Let's check for that.
                    if Self::READ_MODE == Some(PipeMode::Messages) {
                        let has_msg_boundaries = sync_pipeops.does_pipe_have_message_boundaries()
                        .expect("\
failed to determine whether the pipe preserves message boundaries");
                        assert!(has_msg_boundaries, "\
stream wrapper type uses a message-based read mode, but the underlying pipe does not preserve \
message boundaries");
                    }

                    let pipeops = PipeOps::from_sync_pipeops(sync_pipeops)?;

                    let instance = Instance::new(pipeops);
                    Ok(Self { instance })
                }
                $($extra_methods)*
            },
            doc: $doc
        );
        impl TokioPipeStream for $ty {
            const ROLE: PipeStreamRole = $role;
            const WRITE_MODE: Option<PipeMode> = $write_mode;
            const READ_MODE: Option<PipeMode> = $read_mode;
        }
    };
    ($(
        $ty:ident:
            desired_access: $desired_access:expr,
            role: $role:expr,
            read_mode: $read_mode:expr,
            write_mode: $write_mode:expr,
            extra_methods: {$($extra_methods:tt)*},
            doc: $doc:tt
    )+) => {
        $(create_stream_type!(
            $ty:
                desired_access: $desired_access,
                role: $role,
                read_mode: $read_mode,
                write_mode: $write_mode,
                extra_methods: {$($extra_methods)*},
                doc: $doc
        );)+
    };
}
macro_rules! create_duplex_stream_type {
    (
        $ty:ident:
            corresponding_reader: $corresponding_reader:ident,
            corresponding_writer: $corresponding_writer:ident,
            doc: $doc:tt
    ) => {
        create_stream_type!(
            $ty:
                desired_access: GENERIC_READ | GENERIC_WRITE,
                role: PipeStreamRole::ReaderAndWriter,
                read_mode: $corresponding_reader::READ_MODE,
                write_mode: $corresponding_writer::WRITE_MODE,
                extra_methods: {
                    // TODO borrowed split
                    /// Splits the duplex stream into its reading and writing half.
                    pub fn split(self) -> ($corresponding_reader, $corresponding_writer) {
                        let self_ = ManuallyDrop::new(self);
                        let reader_half = self_.instance.split();
                        let writer_half = unsafe {
                            // SAFETY: ManuallyDrop precludes double free
                            ptr::read(&self_.instance)
                        };
                        (
                            $corresponding_reader::build(reader_half),
                            $corresponding_writer::build(writer_half),
                        )
                    }
                },
                doc: $doc
        );
    };
    ($(
        $ty:ident:
            corresponding_reader: $corresponding_reader:ident,
            corresponding_writer: $corresponding_writer:ident,
            doc: $doc:tt
    )+) => {
        $(create_duplex_stream_type!(
            $ty:
                corresponding_reader: $corresponding_reader,
                corresponding_writer: $corresponding_writer,
                doc: $doc
        );)+
    };
}

create_stream_type! {
    ByteReaderPipeStream:
        desired_access: GENERIC_READ,
        role: PipeStreamRole::Reader,
        read_mode: Some(PipeMode::Bytes),
        write_mode: None,
        extra_methods: {},
        doc: "
[Byte stream reader] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream reader]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
"
    ByteWriterPipeStream:
        desired_access: GENERIC_WRITE,
        role: PipeStreamRole::Writer,
        read_mode: None,
        write_mode: Some(PipeMode::Bytes),
        extra_methods: {},
        doc: "
[Byte stream writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream writer]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
"
    MsgReaderPipeStream:
        desired_access: GENERIC_READ,
        role: PipeStreamRole::Reader,
        read_mode: Some(PipeMode::Messages),
        write_mode: None,
        extra_methods: {},
        doc: "
[Message stream reader] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream reader]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
"
    MsgWriterPipeStream:
        desired_access: GENERIC_WRITE,
        role: PipeStreamRole::Writer,
        read_mode: None,
        write_mode: Some(PipeMode::Messages),
        extra_methods: {},
        doc: "
[Message stream writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream writer]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
"
}
create_duplex_stream_type! {
    DuplexBytePipeStream:
        corresponding_reader: ByteReaderPipeStream,
        corresponding_writer: ByteWriterPipeStream,
        doc: "
Byte stream [reader] and [writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
[writer]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
"
    DuplexMsgPipeStream:
        corresponding_reader: MsgReaderPipeStream,
        corresponding_writer: MsgWriterPipeStream,
        doc: "
Message stream [reader] and [writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
[writer]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
"
}

#[cfg(feature = "tokio_support")]
impl AsyncRead for ByteReaderPipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
#[cfg(feature = "tokio_support")]
impl AsyncRead for &ByteReaderPipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}

#[cfg(feature = "tokio_support")]
impl AsyncWrite for ByteWriterPipeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}
#[cfg(feature = "tokio_support")]
impl AsyncWrite for &ByteWriterPipeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}

impl AsyncRead for DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
impl AsyncWrite for DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}
impl AsyncRead for &DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
impl AsyncWrite for &DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}

impl AsyncRead for MsgReaderPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
impl AsyncRead for &MsgReaderPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}

impl AsyncWrite for MsgWriterPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}
impl AsyncWrite for &MsgWriterPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}

impl AsyncRead for DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
impl AsyncWrite for DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}
impl AsyncRead for &DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_read(ctx, buf)
    }
}
impl AsyncWrite for &DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.ops().poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.ops().poll_shutdown(ctx)
    }
}

fn _connect(
    pipe_name: &OsStr,
    hostname: Option<&OsStr>,
    read: bool,
    write: bool,
) -> io::Result<PipeOps> {
    let name = convert_path(pipe_name, hostname);
    let name = OsString::from_wide(&name[..]);
    let name_ref: &OsStr = name.as_ref();
    let result = TokioNPClientOptions::new()
        .read(read)
        .write(write)
        .open(name_ref);
    let client = match result {
        Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
            Err(io::ErrorKind::WouldBlock.into())
        }
        els => els,
    }?;
    let ops = PipeOps::Client(client);
    Ok(ops)
}
// TODO connect with wait
