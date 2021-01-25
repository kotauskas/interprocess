//! Support for named pipes on Windows.
//!
//! ## Windows named pipes are not Unix named pipes
//! The term "named pipe" refers to completely different things in Unix and Windows. For this reason, Unix named pipes are referred to as "FIFO files" to avoid confusion with the more powerful Windows named pipes. In fact, the only common features for those two is that they both can be located using filesystem paths and they both use a stream interface. The simplest way to explain their differences is using a list:
//! - Windows named pipes are located on a separate filesystem (NPFS — **N**amed **P**ipe **F**ile**s**ystem), while Unix FIFO files live in the shared filesystem tree together with all other files
//!     - On Linux, the implementation of Unix domain sockets exposes a similar feature: by setting the first byte in the socket file path to `NULL` (`\0`), the socket is placed into a separate namespace instead of being placed on the filesystem; this is non-standard extension to POSIX and is not available on other Unix systems
//! - Windows named pipes have a server and an arbitrary number of clients, meaning that the separate processes connecting to a named pipe have separate connections to the server, while Unix FIFO files don't have the notion of a server or client and thus mix all data written into one sink from which the data is read by one process
//! - Windows named pipes can be used over the network, while a Unix FIFO file is still local even if created in a directory which is a mounted network filesystem
//! - Windows named pipes can maintain datagram boundaries, allowing both sides of the connection to operate on separate messages rather than on a byte stream, while FIFO files, like any other type of file, expose only a byte stream interface
//!
//! If you carefully read through this list, you'd notice how Windows named pipes are similar to Unix domain sockets. For this reason, the implementation of "local sockets" in the `local_socket` module of this crate uses named pipes on Windows and Ud-sockets on Unix.

// TODO improve docs, add examples

#[cfg(windows)]
use std::os::windows::{
    ffi::OsStrExt,
    io::{AsRawHandle, FromRawHandle, IntoRawHandle},
};
use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
    ffi::{OsStr, OsString},
    fmt::{self, Debug, Formatter},
    io::{self, Read, Write},
    marker::PhantomData,
    mem,
    num::{NonZeroU32, NonZeroU8},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};
#[cfg(not(windows))]
#[doc(hidden)]
pub trait AsRawHandle {}
#[cfg(windows)]
use winapi::{
    shared::minwindef::DWORD,
    um::{
        fileapi::{CreateFileW, OPEN_EXISTING},
        handleapi::INVALID_HANDLE_VALUE,
        namedpipeapi::CreateNamedPipeW,
        winbase::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX, PIPE_ACCESS_INBOUND,
            PIPE_ACCESS_OUTBOUND, PIPE_READMODE_BYTE, PIPE_READMODE_MESSAGE, PIPE_TYPE_BYTE,
            PIPE_TYPE_MESSAGE,
        },
        winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE, HANDLE},
    },
};
#[cfg(not(windows))]
macro_rules! fake_consts {
    ($($name:ident = $val:expr),+ $(,)?) => (
        $(
            #[cfg(not(windows))]
            const $name : u32 = $val;
        )+
    );
}
#[cfg(not(windows))]
fake_consts! {
    PIPE_ACCESS_INBOUND = 0, PIPE_ACCESS_OUTBOUND = 1, PIPE_ACCESS_DUPLEX = 2,
    PIPE_TYPE_BYTE = 1, PIPE_TYPE_MESSAGE = 2,
}
#[cfg(not(windows))]
#[doc(hidden)]
pub type DWORD = u32;
use crate::{PartialMsgWriteError, ReliableReadMsg, Sealed};

fn convert_path(osstr: &OsStr) -> Vec<u16> {
    let mut path = OsString::from(r"\\.\pipe\");
    path.push(osstr);
    let mut path = path.encode_wide().collect::<Vec<u16>>();
    path.push(0); // encode_wide does not include the terminating NULL, so we have to add it ourselves
    path
}

/// The server for a named pipe, listening for connections to clients and producing pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
///
/// [`PipeListenerOptions`]: struct.PipeListenerOptions.html " "
pub struct PipeListener<Stream: PipeStream> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    instances: RwLock<Vec<Arc<(PipeOps, AtomicBool)>>>,
    _phantom: PhantomData<fn() -> Stream>,
}
/// An iterator that infinitely [`accept`]s connections on a [`PipeListener`].
///
/// This iterator is created by the [`incoming`] method on [`PipeListener`]. See its documentation for more.
///
/// [`PipeListener`]: struct.PipeListener.html " "
/// [`accept`]: struct.PipeListener.html#method.accept " "
/// [`incoming`]: struct.PipeListener.html#method.incoming " "
pub struct Incoming<'a, Stream: PipeStream> {
    listener: &'a PipeListener<Stream>,
}
impl<'a, Stream: PipeStream> Iterator for Incoming<'a, Stream> {
    type Item = io::Result<Stream>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
}
impl<'a, Stream: PipeStream> IntoIterator for &'a PipeListener<Stream> {
    type IntoIter = Incoming<'a, Stream>;
    type Item = <Incoming<'a, Stream> as Iterator>::Item;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.incoming()
    }
}
impl<Stream: PipeStream> PipeListener<Stream> {
    /// Blocks until a client connects to the named pipe, creating a `Stream` to communicate with the pipe.
    ///
    /// See `incoming` for an iterator version of this.
    #[inline]
    pub fn accept(&self) -> io::Result<Stream> {
        let instance = self.alloc_instance()?;
        instance.0.connect()?;
        Ok(Stream::from(instance))
    }
    /// Creates an iterator which accepts connections from clients, blocking each time `next()` is called until one connects.
    #[inline]
    pub fn incoming(&self) -> Incoming<'_, Stream> {
        Incoming { listener: self }
    }

    /// Returns a pipe instance either by using an existing one or returning a newly created instance using `add_instance`.
    fn alloc_instance(&self) -> io::Result<Arc<(PipeOps, AtomicBool)>> {
        let instances = self.instances.read().expect("unexpected lock poison");
        for inst in instances.iter() {
            // Try to ownership for the instance by doing a combined compare+exchange, just
            // like a mutex does.
            let cmpxchg_result =
                inst.1
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed);
            if cmpxchg_result.is_ok() {
                // If the compare+exchange returned Ok, then we successfully took ownership of the
                // pipe instance and we can return it right away.
                return Ok(Arc::clone(inst));
            }
            // If not, the pipe we tried to claim is already at work and we need to seek a new
            // one, which is what the next iteration will do.
        }
        // If we searched through the entire thing and never found a free pipe, there isn't one
        // available, which is why we need a new one.
        let new_inst = self.add_instance()?;
        mem::drop(instances); // Get rid of the old lock to lock again with write access.
        let mut instances = self.instances.write().expect("unexpected lock poison");
        instances.push(Arc::clone(&new_inst));
        Ok(new_inst)
    }
    /// Increases instance count by 1 and returns the created instance.
    #[inline]
    fn add_instance(&self) -> io::Result<Arc<(PipeOps, AtomicBool)>> {
        let new_instance = Arc::new(self.config.create_instance::<Stream>(false)?);
        let mut instances = self.instances.write().expect("unexpected lock poison");
        instances.push(Arc::clone(&new_instance));
        Ok(new_instance)
    }
}
mod pipe_listener_debug_impl {
    #[cfg(windows)]
    use super::AsRawHandle;
    use super::{
        fmt, Arc, AtomicBool, Debug, Formatter, Ordering, PipeListener, PipeOps, PipeStream, RwLock,
    };
    /// Shim used to improve pipe instance formatting
    struct Instance<'a> {
        instance: &'a (PipeOps, AtomicBool),
    }
    impl<'a> Debug for Instance<'a> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("PipeInstance")
                .field("handle", &self.instance.0.as_raw_handle())
                .field("connected", &self.instance.1.load(Ordering::Relaxed))
                .finish()
        }
    }
    /// Another shim which uses the Instance shim to print each of the instances as a list
    struct Instances<'a> {
        instances: &'a RwLock<Vec<Arc<(PipeOps, AtomicBool)>>>,
    }
    impl<'a> Debug for Instances<'a> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut list_builder = f.debug_list();
            for instance in self
                .instances
                .read()
                .expect("unexpected lock poisoning")
                .iter()
            {
                list_builder.entry(&Instance { instance });
            }
            list_builder.finish()
        }
    }
    impl<Stream: PipeStream> Debug for PipeListener<Stream> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("PipeListener")
                .field("config", &self.config)
                .field(
                    "instances",
                    &Instances {
                        instances: &self.instances,
                    },
                )
                .finish()
        }
    }
}

// SAFETY: all fields are Send and Sync except for the PhantomData
//unsafe impl<Stream> Send for PipeListener<Stream> {}
//unsafe impl<Stream> Sync for PipeListener<Stream> {}

use seal::*;
mod seal {
    use super::super::FileHandleOps;
    #[cfg(windows)]
    use std::{
        io,
        os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle},
        ptr,
        sync::atomic::AtomicBool,
    };
    #[cfg(windows)]
    use winapi::{
        shared::{minwindef::DWORD, winerror::ERROR_PIPE_CONNECTED},
        um::{
            fileapi::ReadFile,
            namedpipeapi::{ConnectNamedPipe, DisconnectNamedPipe, PeekNamedPipe},
            winbase::{
                GetNamedPipeClientProcessId, GetNamedPipeClientSessionId,
                GetNamedPipeServerProcessId, GetNamedPipeServerSessionId,
            },
            winnt::HANDLE,
        },
    };

    pub trait NamedPipeStreamInternals: From<std::sync::Arc<(super::PipeOps, AtomicBool)>> {}

    /// The actual implementation of a named pipe server or client.
    #[repr(transparent)]
    pub struct PipeOps(pub(super) FileHandleOps);
    impl PipeOps {
        /// Reads a message from the pipe instance into the specified buffer, returning the size of the message written as `Ok(Ok(...))`. If the buffer is too small to fit the message, a bigger buffer is allocated and returned as `Ok(Err(...))`, with the exact size and capacity to hold the message. Errors are returned as `Err(Err(...))`.
        #[inline]
        pub(super) fn read_msg(&self, buf: &mut [u8]) -> io::Result<Result<usize, Vec<u8>>> {
            match self.try_read_msg(buf)? {
                Ok(bytes_read) => Ok(Ok(bytes_read)),
                Err(bytes_left_in_message) => {
                    let mut new_buffer = vec![0; bytes_left_in_message];
                    let mut _number_of_bytes_read: DWORD = 0;
                    let success = unsafe {
                        ReadFile(
                            self.as_raw_handle(),
                            new_buffer.as_mut_slice().as_mut_ptr() as *mut _,
                            buf.len() as DWORD,
                            &mut _number_of_bytes_read as *mut _,
                            ptr::null_mut(),
                        ) != 0
                    };
                    if success {
                        Ok(Err(new_buffer))
                    } else {
                        Err(io::Error::last_os_error())
                    }
                }
            }
        }
        pub(super) fn try_read_msg(&self, buf: &mut [u8]) -> io::Result<Result<usize, usize>> {
            debug_assert!(
                buf.len() <= DWORD::max_value() as usize,
                "buffer is bigger than maximum buffer size for ReadFile",
            );
            let bytes_left_in_message = unsafe {
                let mut bytes_left_in_message: DWORD = 0;
                let result = PeekNamedPipe(
                    self.as_raw_handle(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut bytes_left_in_message as *mut _,
                );
                if result == 0 {
                    return Err(io::Error::last_os_error());
                }
                bytes_left_in_message as usize
            };
            if buf.len() >= bytes_left_in_message {
                // We already know the exact size of the message which is why this does not matter
                let mut _number_of_bytes_read: DWORD = 0;
                let success = unsafe {
                    ReadFile(
                        self.as_raw_handle(),
                        buf.as_mut_ptr() as *mut _,
                        buf.len() as DWORD,
                        &mut _number_of_bytes_read as *mut _,
                        ptr::null_mut(),
                    ) != 0
                };
                if success {
                    Ok(Ok(bytes_left_in_message))
                } else {
                    Err(io::Error::last_os_error())
                }
            } else {
                Ok(Err(bytes_left_in_message))
            }
        }
        /// Reads bytes from the named pipe. Mirrors `std::io::Read`.
        #[inline]
        pub(super) fn read_bytes(&self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
        /// Writes data to the named pipe. There is no way to check/ensure that the message boundaries will be preserved which is why there's only one function to do this.
        #[inline]
        pub(super) fn write(&self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }
        /// Blocks until the client has fully read the buffer.
        #[inline]
        pub(super) fn flush(&self) -> io::Result<()> {
            self.0.flush()
        }

        pub(super) fn get_client_process_id(&self) -> io::Result<u32> {
            let mut id: u32 = 0;
            let success = unsafe { GetNamedPipeClientProcessId(self.0 .0, &mut id as *mut _) != 0 };
            if success {
                Ok(id)
            } else {
                Err(io::Error::last_os_error())
            }
        }
        pub(super) fn get_client_session_id(&self) -> io::Result<u32> {
            let mut id: u32 = 0;
            let success = unsafe { GetNamedPipeClientSessionId(self.0 .0, &mut id as *mut _) != 0 };
            if success {
                Ok(id)
            } else {
                Err(io::Error::last_os_error())
            }
        }
        pub(super) fn get_server_process_id(&self) -> io::Result<u32> {
            let mut id: u32 = 0;
            let success = unsafe { GetNamedPipeServerProcessId(self.0 .0, &mut id as *mut _) != 0 };
            if success {
                Ok(id)
            } else {
                Err(io::Error::last_os_error())
            }
        }
        pub(super) fn get_server_session_id(&self) -> io::Result<u32> {
            let mut id: u32 = 0;
            let success = unsafe { GetNamedPipeServerSessionId(self.0 .0, &mut id as *mut _) != 0 };
            if success {
                Ok(id)
            } else {
                Err(io::Error::last_os_error())
            }
        }

        /// Blocks until connected. If connected, does not do anything.
        pub(super) fn connect(&self) -> io::Result<()> {
            let success = unsafe { ConnectNamedPipe(self.as_raw_handle(), ptr::null_mut()) != 0 };
            if success {
                Ok(())
            } else {
                let last_error = io::Error::last_os_error();
                if last_error.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32) {
                    Ok(())
                } else {
                    Err(last_error)
                }
            }
        }
        /// Flushes and disconnects, obviously.
        #[inline]
        pub(super) fn flush_and_disconnect(&self) -> io::Result<()> {
            self.flush()?;
            self.disconnect()?;
            Ok(())
        }
        /// Disconnects without flushing. Drops all data which has been sent but not yet received on the other side, if any.
        #[inline]
        pub(super) fn disconnect(&self) -> io::Result<()> {
            let success = unsafe { DisconnectNamedPipe(self.as_raw_handle()) != 0 };
            if success {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
    #[cfg(windows)]
    impl AsRawHandle for PipeOps {
        #[inline]
        fn as_raw_handle(&self) -> HANDLE {
            self.0 .0 // I hate this nested tuple syntax.
        }
    }
    #[cfg(windows)]
    impl IntoRawHandle for PipeOps {
        #[inline]
        fn into_raw_handle(self) -> HANDLE {
            let handle = self.as_raw_handle();
            std::mem::forget(self);
            handle
        }
    }
    #[cfg(windows)]
    impl FromRawHandle for PipeOps {
        unsafe fn from_raw_handle(handle: HANDLE) -> Self {
            Self(FileHandleOps::from_raw_handle(handle))
        }
    }
    // SAFETY: we don't expose reading/writing for immutable references of PipeInstance
    unsafe impl Sync for PipeOps {}
    unsafe impl Send for PipeOps {}
}
/// The builder which can be used to create `PipeListener`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct PipeListenerOptions<'a> {
    /// Specifies the name for the named pipe. Since the name typically, but not always, is a string literal, an owned string does not need to be provided.
    pub name: Cow<'a, OsStr>,
    /// Specifies how data is written into the data stream. This is required in all cases, regardless of whether the pipe is inbound, outbound or duplex, since this affects all data being written into the pipe, not just the data written by the server.
    pub mode: PipeMode,
    /// Specifies the maximum amount of instances of the pipe which can be created, i.e. how many clients can be communicated with at once. If set to 1, trying to create multiple instances at the same time will return an error. If set to `None`, no limit is applied. The value 255 is not allowed because of Windows limitations.
    pub instance_limit: Option<NonZeroU8>,
    /// Enables write-through mode, which applies only to network connections to the pipe. If enabled, writing to the pipe would always block until all data is delivered to the other end instead of piling up in the kernel's network buffer until a certain amount of data accamulates or a certain period of time passes, which is when the system actually sends the contents of the buffer over the network.
    ///
    /// Not required for pipes which are restricted to local connections only. If debug assertions are enabled, setting this parameter on a local-only pipe will cause a panic when the pipe is created; in release builds, creation will successfully complete without any errors and the flag will be completely ignored.
    pub write_through: bool,
    /// Enables remote machines to connect to the named pipe over the network.
    pub accept_remote: bool,
    /// Specifies how big the input buffer should be. The system will automatically adjust this size to align it as required or clip it by the minimum or maximum buffer size.
    ///
    /// Not required for outbound pipes and required for inbound and duplex pipes. If debug assertions are enabled, setting this parameter on an outbound pipe will cause a panic when the pipe is created; in release builds, creation will successfully complete without any errors and the value will be completely ignored.
    pub input_buffer_size_hint: usize,
    /// Specifies how big the output buffer should be. The system will automatically adjust this size to align it as required or clip it by the minimum or maximum buffer size.
    ///
    /// Not required for inbound pipes and required for outbound and duplex pipes. If debug assertions are enabled, setting this parameter on an inbound pipe will cause a panic when the pipe is created; in release builds, creation will successfully complete without any errors and the value will be completely ignored.
    pub output_buffer_size_hint: usize,
    /// The default timeout when waiting for a client to connect. Used unless another timeout is specified when waiting for a client.
    pub wait_timeout: NonZeroU32,
}
impl<'a> PipeListenerOptions<'a> {
    /// Creates a new builder with default options.
    #[inline]
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed(OsStr::new("")),
            mode: PipeMode::Bytes,
            instance_limit: None,
            write_through: false,
            accept_remote: false,
            input_buffer_size_hint: 512,
            output_buffer_size_hint: 512,
            wait_timeout: unsafe { NonZeroU32::new_unchecked(50) },
        }
    }
    /// Sets the [`name`] parameter to the specified value.
    ///
    /// [`name`]: #structfield.name " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn name(mut self, name: impl Into<Cow<'a, OsStr>>) -> Self {
        self.name = name.into();
        self
    }
    /// Sets the [`mode`] parameter to the specified value.
    ///
    /// [`mode`]: #structfield.mode " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn mode(mut self, mode: PipeMode) -> Self {
        self.mode = mode;
        self
    }
    /// Sets the [`instance_limit`] parameter to the specified value.
    ///
    /// [`instance_limit`]: #structfield.instance_limit " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn instance_limit(mut self, instance_limit: impl Into<Option<NonZeroU8>>) -> Self {
        self.instance_limit = instance_limit.into();
        self
    }
    /// Sets the [`write_through`] parameter to the specified value.
    ///
    /// [`write_through`]: #structfield.write_through " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn write_through(mut self, write_through: bool) -> Self {
        self.write_through = write_through;
        self
    }
    /// Sets the [`accept_remote`] parameter to the specified value.
    ///
    /// [`accept_remote`]: #structfield.accept_remote " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn accept_remote(mut self, accept_remote: bool) -> Self {
        self.accept_remote = accept_remote;
        self
    }
    /// Sets the [`input_buffer_size_hint`] parameter to the specified value.
    ///
    /// [`input_buffer_size_hint`]: #structfield.input_buffer_size_hint " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn input_buffer_size_hint(mut self, input_buffer_size_hint: impl Into<usize>) -> Self {
        self.input_buffer_size_hint = input_buffer_size_hint.into();
        self
    }
    /// Sets the [`output_buffer_size_hint`] parameter to the specified value.
    ///
    /// [`output_buffer_size_hint`]: #structfield.output_buffer_size_hint " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn output_buffer_size_hint(mut self, output_buffer_size_hint: impl Into<usize>) -> Self {
        self.output_buffer_size_hint = output_buffer_size_hint.into();
        self
    }
    /// Sets the [`wait_timeout`] parameter to the specified value.
    ///
    /// [`wait_timeout`]: #structfield.wait_timeout " "
    #[inline]
    #[must_use = "builder setters take the entire structure and return the result"]
    pub fn wait_timeout(mut self, wait_timeout: impl Into<NonZeroU32>) -> Self {
        self.wait_timeout = wait_timeout.into();
        self
    }
    /// Creates an instance of a pipe for a listener with the specified stream type and with the first-instance flag set to the specified value.
    fn create_instance<Stream: PipeStream>(
        &self,
        first: bool,
    ) -> io::Result<(PipeOps, AtomicBool)> {
        let path = convert_path(&self.name);
        let (handle, success) = unsafe {
            let handle = CreateNamedPipeW(
                path.as_ptr(),
                {
                    let mut flags = DWORD::from(Stream::ROLE.direction_as_server());
                    if first {
                        flags |= FILE_FLAG_FIRST_PIPE_INSTANCE;
                    }
                    flags
                },
                self.mode.to_pipe_type() | Stream::READ_MODE.map_or(0, |x| x.to_readmode()),
                self.instance_limit.map_or(255, |x| {
                    assert!(x.get() != 255, "cannot set 255 as the named pipe instance limit due to 255 being a reserved value");
                    x.get() as DWORD
                }),
                self.output_buffer_size_hint.try_into()
                    .expect("output buffer size hint overflowed DWORD"),
                self.input_buffer_size_hint.try_into()
                    .expect("input buffer size hint overflowed DWORD"),
                self.wait_timeout.get(),
                ptr::null_mut(),
            );
            (handle, handle != INVALID_HANDLE_VALUE)
        };
        if success {
            // SAFETY: we just created this handle
            Ok((
                unsafe { PipeOps::from_raw_handle(handle) },
                AtomicBool::new(false),
            ))
        } else {
            Err(io::Error::last_os_error())
        }
    }
    /// Creates the pipe listener from the builder. The `Stream` generic argument specifies the type of pipe stream that the listener will create, thus determining the direction of the pipe and its mode.
    ///
    /// For outbound or duplex pipes, the `mode` parameter must agree with the `Stream`'s `WRITE_MODE`. Otherwise, the call will panic in debug builds or, in release builds, the `WRITE_MODE` will take priority.
    #[inline]
    pub fn create<Stream: PipeStream>(&self) -> io::Result<PipeListener<Stream>> {
        // We need this ugliness because the compiler does not understand that
        // PipeListenerOptions<'a> can coerce into PipeListenerOptions<'static> if we manually
        // replace the name field with Cow::Owned and just copy all other elements over thanks
        // to the fact that they don't contain a mention of the lifetime 'a. Tbh we need an
        // RFC for this, would be nice.
        let owned_config = PipeListenerOptions {
            name: Cow::Owned(self.name.clone().into_owned()),
            mode: self.mode,
            instance_limit: self.instance_limit,
            write_through: self.write_through,
            accept_remote: self.accept_remote,
            input_buffer_size_hint: self.input_buffer_size_hint,
            output_buffer_size_hint: self.output_buffer_size_hint,
            wait_timeout: self.wait_timeout,
        };
        Ok(PipeListener {
            config: owned_config,
            instances: RwLock::new({
                let capacity = self.instance_limit.map_or(8, |x| x.get()) as usize;
                let mut vec = Vec::with_capacity(capacity);
                vec.push(Arc::new(self.create_instance::<Stream>(true)?));
                vec
            }),
            _phantom: PhantomData,
        })
    }
}
impl Default for PipeListenerOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! create_stream_type {
    ($($ty:ident, $desired_access:expr, doc: $doc:tt)+) => ($(
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
        impl Sealed for $ty {}
        impl NamedPipeStreamInternals for $ty {}
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
        #[doc(hidden)]
        impl From<Arc<(PipeOps, AtomicBool)>> for $ty {
            #[inline]
            fn from(instance: Arc<(PipeOps, AtomicBool)>) -> Self {
                Self {instance}
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
                Self {
                    instance: Arc::new((
                        PipeOps::from_raw_handle(handle),
                        AtomicBool::new(false),
                    ))
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
    ByteReaderPipeStream, GENERIC_READ, doc: "
[Byte stream reader] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
"
    ByteWriterPipeStream, GENERIC_WRITE, doc: "
[Byte stream writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Byte stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    DuplexBytePipeStream, GENERIC_READ | GENERIC_WRITE, doc: "
Byte stream [reader] and [writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[reader]: https://doc.rust-lang.org/std/io/trait.Read.html
[writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    MsgReaderPipeStream, GENERIC_READ, doc: "
[Message stream reader] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream reader]: https://doc.rust-lang.org/std/io/trait.Read.html
"
    MsgWriterPipeStream, GENERIC_WRITE, doc: "
[Message stream writer] for a named pipe.

Created either by using `PipeListener` or by connecting to a named pipe server.

[Message stream writer]: https://doc.rust-lang.org/std/io/trait.Write.html
"
    DuplexMsgPipeStream, GENERIC_READ | GENERIC_WRITE, doc: "
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
pub trait PipeStream: AsRawHandle + NamedPipeStreamInternals {
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
impl PipeStream for ByteReaderPipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::Reader;
    const WRITE_MODE: Option<PipeMode> = None;
    const READ_MODE: Option<PipeMode> = Some(PipeMode::Bytes);
}
impl PipeStream for ByteWriterPipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::Writer;
    const WRITE_MODE: Option<PipeMode> = Some(PipeMode::Bytes);
    const READ_MODE: Option<PipeMode> = None;
}
impl PipeStream for DuplexBytePipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::ReaderAndWriter;
    const WRITE_MODE: Option<PipeMode> = Some(PipeMode::Bytes);
    const READ_MODE: Option<PipeMode> = Some(PipeMode::Bytes);
}
impl PipeStream for MsgReaderPipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::Reader;
    const WRITE_MODE: Option<PipeMode> = None;
    const READ_MODE: Option<PipeMode> = Some(PipeMode::Messages);
}
impl PipeStream for MsgWriterPipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::Writer;
    const WRITE_MODE: Option<PipeMode> = Some(PipeMode::Messages);
    const READ_MODE: Option<PipeMode> = None;
}
impl PipeStream for DuplexMsgPipeStream {
    const ROLE: PipeStreamRole = PipeStreamRole::ReaderAndWriter;
    const WRITE_MODE: Option<PipeMode> = Some(PipeMode::Messages);
    const READ_MODE: Option<PipeMode> = Some(PipeMode::Messages);
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
        Ok(Stream::from(Arc::from((
            unsafe { PipeOps::from_raw_handle(handle) }, // SAFETY: we just created this handle
            AtomicBool::new(true),
        ))))
    } else {
        Err(io::Error::last_os_error())
    }
}

/// The direction of a named pipe connection, designating who can read data and who can write it. This describes the direction of the data flow unambiguously, so that the meaning of the values is the same for the client and server — [`ClientToServer`] always means client → server, for example.
///
/// [`ClientToServer`]: enum.PipeDirection.html#variant.ClientToServer " "
// I had to type out both the link to the page and the name of the variant since the link can be clicked from module-level documentation so please don't touch it.
#[repr(u32)]
// We depend on the fact that DWORD always maps to u32, which, thankfully, will always stay true
// since the public WinAPI is supposed to be ABI-compatible. Just keep in mind that the
// #[repr(u32)] means that we can transmute this enumeration to the Windows DWORD type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PipeDirection {
    /// Represents a server ← client data flow: clients write data, the server reads it.
    ClientToServer = PIPE_ACCESS_INBOUND,
    /// Represents a server → client data flow: the server writes data, clients read it.
    ServerToClient = PIPE_ACCESS_OUTBOUND,
    /// Represents a server ⇄ client data flow: the server can write data which then is read by the client, while the client writes data which is read by the server.
    Duplex = PIPE_ACCESS_DUPLEX,
}
impl PipeDirection {
    /// Returns the role which the pipe client will have in this direction setting.
    ///
    /// # Usage
    /// ```
    /// # #[cfg(windows)] {
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeDirection::ClientToServer.client_role(),
    ///     PipeStreamRole::Writer,
    /// );
    /// assert_eq!(
    ///     PipeDirection::ServerToClient.client_role(),
    ///     PipeStreamRole::Reader,
    /// );
    /// assert_eq!(
    ///     PipeDirection::Duplex.client_role(),
    ///     PipeStreamRole::ReaderAndWriter,
    /// );
    /// # }
    /// ```
    #[inline]
    pub fn client_role(self) -> PipeStreamRole {
        match self {
            Self::ClientToServer => PipeStreamRole::Writer,
            Self::ServerToClient => PipeStreamRole::Reader,
            Self::Duplex => PipeStreamRole::ReaderAndWriter,
        }
    }
    /// Returns the role which the pipe server will have in this direction setting.
    ///
    /// # Usage
    /// ```
    /// # #[cfg(windows)] {
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeDirection::ClientToServer.server_role(),
    ///     PipeStreamRole::Reader,
    /// );
    /// assert_eq!(
    ///     PipeDirection::ServerToClient.server_role(),
    ///     PipeStreamRole::Writer,
    /// );
    /// assert_eq!(
    ///     PipeDirection::Duplex.server_role(),
    ///     PipeStreamRole::ReaderAndWriter,
    /// );
    /// # }
    /// ```
    #[inline]
    pub fn server_role(self) -> PipeStreamRole {
        match self {
            Self::ClientToServer => PipeStreamRole::Reader,
            Self::ServerToClient => PipeStreamRole::Writer,
            Self::Duplex => PipeStreamRole::ReaderAndWriter,
        }
    }
}
impl TryFrom<DWORD> for PipeDirection {
    type Error = ();
    /// Converts a Windows constant to a `PipeDirection` if it's in range.
    ///
    /// # Errors
    /// Returns `Err` if the value is not a valid pipe direction constant.
    #[inline]
    fn try_from(op: DWORD) -> Result<Self, ()> {
        assert!((1..=3).contains(&op));
        // See the comment block above for why this is safe.
        unsafe { mem::transmute(op) }
    }
}
impl From<PipeDirection> for DWORD {
    #[inline]
    fn from(op: PipeDirection) -> Self {
        unsafe { mem::transmute(op) }
    }
}
/// Describes the role of a named pipe stream. In constrast to [`PipeDirection`], the meaning of values here is relative — for example, [`Reader`] means [`ServerToClient`] if you're creating a server and [`ClientToServer`] if you're creating a client.
///
/// This enumeration is also not layout-compatible with the `PIPE_ACCESS_*` constants, in contrast to [`PipeDirection`].
///
/// [`PipeDirection`]: enum.PipeDirection.html " "
/// [`Reader`]: #variant.Reader " "
/// [`ServerToClient`]: enum.PipeDirection.html#variant.ServerToClient " "
/// [`ClientToServer`]: enum.PipeDirection.html#variant.ClientToServer " "
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PipeStreamRole {
    /// The stream only reads data.
    Reader,
    /// The stream only writes data.
    Writer,
    /// The stream both reads and writes data.
    ReaderAndWriter,
}
impl PipeStreamRole {
    /// Returns the data flow direction of the data stream, assuming that the value describes the role of the server.
    ///
    /// # Usage
    /// ```
    /// # #[cfg(windows)] {
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeStreamRole::Reader.direction_as_server(),
    ///     PipeDirection::ClientToServer,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::Writer.direction_as_server(),
    ///     PipeDirection::ServerToClient,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::ReaderAndWriter.direction_as_server(),
    ///     PipeDirection::Duplex,
    /// );
    /// # }
    /// ```
    #[inline]
    pub fn direction_as_server(self) -> PipeDirection {
        match self {
            Self::Reader => PipeDirection::ClientToServer,
            Self::Writer => PipeDirection::ServerToClient,
            Self::ReaderAndWriter => PipeDirection::Duplex,
        }
    }
    /// Returns the data flow direction of the data stream, assuming that the value describes the role of the client.
    ///
    /// # Usage
    /// ```
    /// # #[cfg(windows)] {
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeStreamRole::Reader.direction_as_client(),
    ///     PipeDirection::ServerToClient,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::Writer.direction_as_client(),
    ///     PipeDirection::ClientToServer,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::ReaderAndWriter.direction_as_client(),
    ///     PipeDirection::Duplex,
    /// );
    /// # }
    /// ```
    #[inline]
    pub fn direction_as_client(self) -> PipeDirection {
        match self {
            Self::Reader => PipeDirection::ServerToClient,
            Self::Writer => PipeDirection::ClientToServer,
            Self::ReaderAndWriter => PipeDirection::Duplex,
        }
    }
}

/// Specifies the mode for a pipe stream.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PipeMode {
    /// Designates that the pipe stream works in byte stream mode, erasing the boundaries of separate messages.
    Bytes = PIPE_TYPE_BYTE,
    /// Designates that the pipe stream works in message stream mode, preserving the boundaries of separate messages yet still allowing to read them in byte stream mode.
    Messages = PIPE_TYPE_MESSAGE,
}
impl PipeMode {
    /// Converts the value into a raw `DWORD`-typed constant, either `PIPE_TYPE_BYTE` or `PIPE_TYPE_MESSAGE` depending on the value.
    #[inline]
    pub fn to_pipe_type(self) -> DWORD {
        unsafe { mem::transmute(self) } // We already store PIPE_TYPE_*
    }
    /// Converts the value into a raw `DWORD`-typed constant, either `PIPE_READMODE_BYTE` or `PIPE_READMODE_MESSAGE` depending on the value.
    #[inline]
    pub fn to_readmode(self) -> DWORD {
        match self {
            Self::Bytes => PIPE_READMODE_BYTE,
            Self::Messages => PIPE_READMODE_MESSAGE,
        }
    }
}
impl TryFrom<DWORD> for PipeMode {
    type Error = ();
    /// Converts a Windows constant to a `PipeMode` if it's in range. Both `PIPE_TYPE_*` and `PIPE_READMODE_*` are supported.
    ///
    /// # Errors
    /// Returns `Err` if the value is not a valid pipe stream mode constant.
    #[inline]
    fn try_from(op: DWORD) -> Result<Self, ()> {
        // It's nicer to only match than to check and transmute
        match op {
            PIPE_TYPE_BYTE => Ok(Self::Bytes),
            PIPE_READMODE_MESSAGE | PIPE_TYPE_MESSAGE => Ok(Self::Messages),
            _ => Err(()),
        }
    }
}
