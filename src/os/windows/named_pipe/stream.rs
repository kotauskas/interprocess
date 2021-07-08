use super::{
    super::{imports::*, AsRawHandle, FromRawHandle, IntoRawHandle},
    convert_path, set_nonblocking_for_stream, NamedPipeStreamInternals, PipeMode, PipeOps,
    PipeStreamRole,
};
use crate::{PartialMsgWriteError, ReliableReadMsg, Sealed};
use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io::{self, Read, Write},
    mem, ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

macro_rules! create_stream_type {
    ($(
        $ty:ident:
            desired_access: $desired_access:expr,
            role: $role:expr,
            read_mode: $read_mode:expr,
            write_mode: $write_mode:expr,
            doc: $doc:tt
    )+) => ($(
        #[doc = $doc]
        pub struct $ty {
            instance: Arc<(PipeOps, AtomicBool)>,
        }
        impl $ty {
            /// Connects to an existing named pipe.
            #[inline]
            pub fn connect(name: impl AsRef<OsStr>) -> io::Result<Self> {
                let name = convert_path(name.as_ref());
                let (success, handle) = unsafe {
                    let handle = CreateFileW(
                        name.as_ptr() as *mut _,
                        $desired_access,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        ptr::null_mut(),
                        OPEN_EXISTING,
                        0,
                        ptr::null_mut(),
                    );
                    (handle != INVALID_HANDLE_VALUE, handle)
                };
                if success {
                    Ok(unsafe {Self {
                        // SAFETY: we just created this handle, which means that
                        // it's not being used anywhere else
                        instance: Arc::new((
                            PipeOps::from_raw_handle(handle), AtomicBool::new(false),
                        ))
                    }})
                } else {
                    Err(io::Error::last_os_error())
                }
            }
            /// Sets whether the nonblocking mode for the pipe stream is enabled. By default, it is disabled.
            ///
            /// In nonblocking mode, attempts to read from the pipe when there is no data available or to write when the buffer has filled up because the receiving side did not read enough bytes in time will never block like they normally do. Instead, a [`WouldBlock`] error is immediately returned, allowing the thread to perform useful actions in the meantime.
            ///
            /// *If called on the server side, the flag will be set only for one stream instance.* A listener creation option, [`nonblocking`], and a similar method on the listener, [`set_nonblocking`], can be used to set the mode in bulk for all current instances and future ones.
            ///
            /// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock " "
            /// [`nonblocking`]: struct.PipeListenerOptions.html#structfield.nonblocking " "
            /// [`set_nonblocking`]: struct.PipeListener.html#method.set_nonblocking " "
            #[inline]
            pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
                unsafe {
                    set_nonblocking_for_stream::<Self>(self.as_raw_handle(), nonblocking)
                }
            }
            /// Retrieves the process identifier of the client side of the named pipe connection.
            #[inline]
            pub fn client_process_id(&self) -> io::Result<u32> {
                self.instance.0.get_client_process_id()
            }
            /// Retrieves the session identifier of the client side of the named pipe connection.
            #[inline]
            pub fn client_session_id(&self) -> io::Result<u32> {
                self.instance.0.get_client_session_id()
            }
            /// Retrieves the process identifier of the server side of the named pipe connection.
            #[inline]
            pub fn server_process_id(&self) -> io::Result<u32> {
                self.instance.0.get_server_process_id()
            }
            /// Retrieves the session identifier of the server side of the named pipe connection.
            #[inline]
            pub fn server_session_id(&self) -> io::Result<u32> {
                self.instance.0.get_server_session_id()
            }
            /// Disconnects the named pipe stream without flushing buffers, causing all data in those buffers to be lost. This is much faster than simply dropping the stream, since the `Drop` implementation flushes first. Only makes sense for server-side pipes and will panic in debug builds if called on a client stream.
            #[inline]
            pub fn disconnect_without_flushing(self) -> io::Result<()> {
                self.instance.0.disconnect()?;
                // We keep the atomic store anyway since checking whether we're a client or a server and avoiding an atomic write is potentially slower than that write.
                self.instance.1.store(false, Ordering::Release);
                let instance = unsafe {
                    // SAFETY: mem::forget is used to safely destroy the invalidated master copy
                    ptr::read(&self.instance)
                };
                drop(instance);
                mem::forget(self);
                Ok(())
            }
        }
        #[cfg(windows)]
        impl PipeStream for $ty {
            const ROLE: PipeStreamRole = $role;
            const WRITE_MODE: Option<PipeMode> = $write_mode;
            const READ_MODE: Option<PipeMode> = $read_mode;
        }
        impl Sealed for $ty {}
        #[cfg(windows)]
        impl NamedPipeStreamInternals for $ty {
            fn build(instance: Arc<(PipeOps, AtomicBool)>) -> Self {
                Self { instance }
            }
        }
        impl Drop for $ty {
            #[inline]
            fn drop(&mut self) {
                // We can and should ignore the result because we can't handle it from a Drop
                // implementation and we can't/shouldn't handle it either
                let _ = self.instance.0.flush_and_disconnect();
                // See note about atomics above.
                self.instance.1.store(false, Ordering::Release);
            }
        }
        #[cfg(windows)]
        impl AsRawHandle for $ty {
            #[inline]
            fn as_raw_handle(&self) -> HANDLE {
                self.instance.0.as_raw_handle()
            }
        }
        #[cfg(windows)]
        impl IntoRawHandle for $ty {
            #[inline]
            fn into_raw_handle(self) -> HANDLE {
                let handle = self.instance.0.as_raw_handle();
                handle
            }
        }
        #[cfg(windows)]
        impl FromRawHandle for $ty {
            #[inline]
            unsafe fn from_raw_handle(handle: HANDLE) -> Self {
                let pipeops = unsafe {
                    // SAFETY: guaranteed via safety contract
                    PipeOps::from_raw_handle(handle)
                };
                Self {
                    instance: Arc::new((pipeops, AtomicBool::new(false)))
                }
            }
        }
        impl Debug for $ty {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($ty))
                    .field("handle", &self.as_raw_handle())
                    .finish()
            }
        }
    )+);
}
create_stream_type! {
    ByteReaderPipeStream:
        desired_access: GENERIC_READ,
        role: PipeStreamRole::Reader,
        read_mode: Some(PipeMode::Bytes),
        write_mode: None,
        doc: "
[Byte stream reader] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
"
    ByteWriterPipeStream:
        desired_access: GENERIC_WRITE,
        role: PipeStreamRole::Writer,
        read_mode: None,
        write_mode: Some(PipeMode::Bytes),
        doc: "
[Byte stream writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    DuplexBytePipeStream:
        desired_access: GENERIC_READ | GENERIC_WRITE,
        role: PipeStreamRole::ReaderAndWriter,
        read_mode: Some(PipeMode::Bytes),
        write_mode: Some(PipeMode::Bytes),
        doc: "
Byte stream [reader] and [writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://doc.rust-lang.org/std/io/trait.Read.html
[writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    MsgReaderPipeStream:
        desired_access: GENERIC_READ,
        role: PipeStreamRole::Reader,
        read_mode: Some(PipeMode::Messages),
        write_mode: None,
        doc: "
[Message stream reader] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
"
    MsgWriterPipeStream:
        desired_access: GENERIC_WRITE,
        role: PipeStreamRole::Writer,
        read_mode: None,
        write_mode: Some(PipeMode::Messages),
        doc: "
[Message stream writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    DuplexMsgPipeStream:
    desired_access: GENERIC_READ | GENERIC_WRITE,
    role: PipeStreamRole::ReaderAndWriter,
    read_mode: Some(PipeMode::Messages),
    write_mode: Some(PipeMode::Messages),
    doc: "
Message stream [reader] and [writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://doc.rust-lang.org/std/io/trait.Read.html
[writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
}

impl Read for ByteReaderPipeStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.instance.0.read_bytes(buf)
    }
}

impl Write for ByteWriterPipeStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.instance.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.instance.0.flush()
    }
}

impl Read for DuplexBytePipeStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.instance.0.read_bytes(buf)
    }
}
impl Write for DuplexBytePipeStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.instance.0.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.instance.0.flush()
    }
}

impl Read for MsgReaderPipeStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.instance.0.read_bytes(buf)
    }
}
impl ReliableReadMsg for MsgReaderPipeStream {
    #[inline]
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
        self.instance.0.read_msg(buf)
    }
    #[inline]
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
        self.instance.0.try_read_msg(buf)
    }
}

impl Write for MsgWriterPipeStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.instance.0.write(buf)? == buf.len() {
            Ok(buf.len())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, PartialMsgWriteError))
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.instance.0.flush()
    }
}

impl Read for DuplexMsgPipeStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.instance.0.read_bytes(buf)
    }
}
impl ReliableReadMsg for DuplexMsgPipeStream {
    #[inline]
    fn read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
        self.instance.0.read_msg(buf)
    }
    fn try_read_msg(&mut self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
        self.instance.0.try_read_msg(buf)
    }
}
impl Write for DuplexMsgPipeStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.instance.0.write(buf)? == buf.len() {
            Ok(buf.len())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, PartialMsgWriteError))
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.instance.0.flush()
    }
}

/// Defines the properties of pipe stream types.
///
/// ## Why there are multiple types of pipe streams
/// One of the similarities between Unix domain sockets and Windows named pipes is how both can be used in datagram mode and in byte stream mode, that is, like with sockets, Windows named pipes can both maintain the boundaries between packets or erase those boundaries — the specific behavior can be controlled both during pipe creation and during connection. The reader can still use the stream interface even if the writer maintains datagram boundaries, and vice versa: the system automatically disassembles the datagrams into a byte stream with virtually no cost.
///
/// The distinction between datagram-oriented connections and byte streams exists for symmetry with the standard library, where UDP and TCP sockets are represented by different types. The idea behind this is that by separating the two semantic types of sockets into two types, the distinction between those semantics can be enforced at compile time instead of using runtime errors to signal that, for example, a datagram read operation is attempted on a byte stream.
///
/// The fact that named pipes can have different data flow directions further increases the amount of various stream types. By restricting the implemented stream traits at compile time, named pipe streams can be used correctly in generic contexts unaware of named pipes without extra runtime checking for the correct pipe direction.
pub trait PipeStream:
    AsRawHandle + IntoRawHandle + FromRawHandle + NamedPipeStreamInternals
{
    /// The data stream flow direction for the pipe. See the [`PipeStreamRole`] enumeration for more on what this means.
    ///
    /// [`PipeStreamRole`]: enum.PipeStreamRole.html " "
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

/// Connects to the specified named pipe, returning a named pipe stream of the stream type provided via generic parameters.
///
/// Since named pipes can work across multiple machines, an optional hostname can be supplied. Leave it at `None` if you're using named pipes on the local machine exclusively, which is most likely the case.
pub fn connect<Stream: PipeStream>(
    pipe_name: impl AsRef<OsStr>,
    hostname: Option<impl AsRef<OsStr>>,
) -> io::Result<Stream> {
    let mut path = {
        let mut path = OsString::from(r"\\.");
        if let Some(host) = hostname {
            path.push(host);
        } else {
            path.push(".");
        }
        path.push(r"\pipe\");
        path.push(pipe_name.as_ref());
        let mut path = path.encode_wide().collect::<Vec<u16>>();
        path.push(0);
        path
    };
    let (success, handle) = unsafe {
        let handle = CreateFileW(
            path.as_mut_ptr() as *mut _,
            {
                let mut access_flags: DWORD = 0;
                if Stream::READ_MODE.is_some() {
                    access_flags |= GENERIC_READ;
                }
                if Stream::WRITE_MODE.is_some() {
                    access_flags |= GENERIC_WRITE;
                }
                access_flags
            },
            0,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        (handle != INVALID_HANDLE_VALUE, handle)
    };
    if success {
        Ok(Stream::build(Arc::from((
            unsafe { PipeOps::from_raw_handle(handle) }, // SAFETY: we just created this handle
            AtomicBool::new(true),
        ))))
    } else {
        Err(io::Error::last_os_error())
    }
}
