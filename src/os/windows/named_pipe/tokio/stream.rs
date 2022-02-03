use super::{
    super::convert_path,
    enums::{PipeMode, PipeStreamRole},
    imports::*,
    PipeOps, PipeStreamInternals,
};
#[cfg_attr(not(all(windows, feature = "tokio_support")), allow(unused_imports))]
use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io,
    mem::ManuallyDrop,
    pin::Pin,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering::Release},
        Arc, Mutex,
    },
    task::{Context, Poll},
};
use to_method::To;

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
                /// Tries to connect to the specified named pipe (the `\\.\pipe\` prefix is added automatically), returning a named pipe stream of the stream type provided via generic parameters. If there is no available server, returns immediately.
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
                    let instance = (pipeops, AtomicBool::new(false));
                    Ok(Self { instance: Arc::new(instance) })
                }
                /// Tries to connect to the specified named pipe at a remote computer (the `\\<hostname>\pipe\` prefix is added automatically), returning a named pipe stream of the stream type provided via generic parameters. If there is no available server, returns immediately.
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
                    let instance = (pipeops, AtomicBool::new(false));
                    Ok(Self { instance: Arc::new(instance) })
                }
                /// Returns `true` if the stream was created by a listener (server-side), `false` if it was created by connecting to a server (client-side).
                pub fn is_server(&self) -> bool {
                    matches!(self.instance.0, PipeOps::Server(_))
                }
                /// Returns `true` if the stream was created by connecting to a server (client-side), `false` if it was created by a listener (server-side).
                pub fn is_client(&self) -> bool {
                    matches!(self.instance.0, PipeOps::Client(_))
                }
                fn is_split(&self) -> bool {
                    let unsplit_ref_count = if matches!(self.instance.0, PipeOps::Server(_)) {
                        2 // this reference and the listener one
                    } else {
                        1 // only this one
                    };
                    Arc::strong_count(&self.instance) > unsplit_ref_count
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
                    /// Splits the duplex stream into its reading and writing half. Contended concurrent operations may experience insignificant slowdowns due to necessary synchronization, which is an implementation detail.
                    pub fn split(self) -> ($corresponding_reader, $corresponding_writer) {
                        let self_ = ManuallyDrop::new(self);
                        let reader_half = Arc::clone(&self_.instance);
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

[Byte stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
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

[Byte stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
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

[Message stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
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

[Message stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
}
create_duplex_stream_type! {
    DuplexBytePipeStream:
        corresponding_reader: ByteReaderPipeStream,
        corresponding_writer: ByteWriterPipeStream,
        doc: "
Byte stream [reader] and [writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://doc.rust-lang.org/std/io/trait.Read.html
[writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    DuplexMsgPipeStream:
        corresponding_reader: MsgReaderPipeStream,
        corresponding_writer: MsgWriterPipeStream,
        doc: "
Message stream [reader] and [writer] for a Tokio-based named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://doc.rust-lang.org/std/io/trait.Read.html
[writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
}

#[cfg(feature = "tokio_support")]
impl AsyncRead for ByteReaderPipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
#[cfg(feature = "tokio_support")]
impl AsyncRead for &ByteReaderPipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}

#[cfg(feature = "tokio_support")]
impl AsyncWrite for ByteWriterPipeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}
#[cfg(feature = "tokio_support")]
impl AsyncWrite for &ByteWriterPipeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}

impl AsyncRead for DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
impl AsyncWrite for DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}
impl AsyncRead for &DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
impl AsyncWrite for &DuplexBytePipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}

impl AsyncRead for MsgReaderPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
impl AsyncRead for &MsgReaderPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}

impl AsyncWrite for MsgWriterPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}
impl AsyncWrite for &MsgWriterPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}

impl AsyncRead for DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
impl AsyncWrite for DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
    }
}
impl AsyncRead for &DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_read(ctx, buf)
    }
}
impl AsyncWrite for &DuplexMsgPipeStream {
    #[cfg(feature = "tokio_support")]
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.instance.0.poll_write(ctx, buf)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_flush(ctx)
    }
    #[cfg(feature = "tokio_support")]
    fn poll_close(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.instance.0.poll_shutdown(ctx)
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
    let tnpclient = TokioNPClientOptions::new()
        .read(read)
        .write(write)
        .open(name_ref)?;
    let pipeops = PipeOps::Client(tnpclient.to::<Mutex<_>>());
    Ok(pipeops)
}
