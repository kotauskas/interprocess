use super::{
    super::{imports::*, AsRawHandle, FromRawHandle},
    convert_path, set_nonblocking_for_stream, Instancer, PipeMode, PipeOps, PipeStream,
};
use std::{
    borrow::Cow,
    convert::TryInto,
    ffi::OsStr,
    fmt::{self, Debug, Formatter},
    io,
    marker::PhantomData,
    num::{NonZeroU32, NonZeroU8},
    ptr,
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{Relaxed, SeqCst},
        },
        Arc, RwLock,
    },
};

/// The server for a named pipe, listening for connections to clients and producing pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
///
/// [`PipeListenerOptions`]: struct.PipeListenerOptions.html " "
pub struct PipeListener<Stream: PipeStream> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    // Store the nonblocking boolean separately to change it without mutable access
    nonblocking: AtomicBool,
    instancer: Instancer<PipeOps>,
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
        let instance = if let Some(instance) = self.instancer.allocate() {
            instance
        } else {
            self.instancer.add_instance(self.create_instance()?)
        };
        instance.0.connect()?;
        Ok(Stream::build(instance))
    }
    /// Creates an iterator which accepts connections from clients, blocking each time `next()` is called until one connects.
    #[inline]
    pub fn incoming(&self) -> Incoming<'_, Stream> {
        Incoming { listener: self }
    }
    /// Enables or disables the nonblocking mode for all existing instances of the listener and future ones. By default, it is disabled.
    ///
    /// This should ideally be done during creation, using the [`nonblocking` field] of the creation options, unless there's a good reason not to: this method has O(n) complexity, since it has to iterate through all instances, and it may leave the instances in an inconsistent state where some are nonblocking and some are not if the operation fails for one of them (which will be reported as failure, even though some may have successfully been reconfigured).
    ///
    /// See the documentation of the aforementioned field for the exact effects of enabling this mode.
    ///
    /// [`nonblocking` field]: struct.PipeListenerOptions.html#structfield.nonblocking " "
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        for instance in self
            .instancer
            .0
            .read()
            .expect("unexpected lock poison")
            .iter()
        {
            unsafe {
                set_nonblocking_for_stream::<Stream>(instance.0 .0 .0, nonblocking)?;
            }
        }
        self.nonblocking.store(nonblocking, SeqCst);
        Ok(())
    }

    fn create_instance(&self) -> io::Result<PipeOps> {
        self.config
            .create_instance::<Stream>(false, self.nonblocking.load(SeqCst))
    }
}

mod debug_impl {
    use super::*;
    /// Shim used to improve pipe instance formatting
    struct Instance<'a> {
        instance: &'a (PipeOps, AtomicBool),
    }
    impl<'a> Debug for Instance<'a> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("PipeInstance")
                .field("handle", &self.instance.0.as_raw_handle())
                .field("connected", &self.instance.1.load(Relaxed))
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
                        instances: &self.instancer.0,
                    },
                )
                .finish()
        }
    }
}

/// Allows for thorough customization of [`PipeListener`]s during creation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct PipeListenerOptions<'a> {
    /// Specifies the name for the named pipe. Since the name typically, but not always, is a string literal, an owned string does not need to be provided.
    pub name: Cow<'a, OsStr>,
    /// Specifies how data is written into the data stream. This is required in all cases, regardless of whether the pipe is inbound, outbound or duplex, since this affects all data being written into the pipe, not just the data written by the server.
    pub mode: PipeMode,
    /// Specifies whether nonblocking mode will be enabled for all stream instances upon creation. By default, it is disabled.
    ///
    /// There are two ways in which the listener is affected by nonblocking mode:
    /// - Whenever [`accept`] is called or [`incoming`] is being iterated through, if there is no client currently attempting to connect to the named pipe server, the method will return immediately with the [`WouldBlock`] error instead of blocking until one arrives.
    /// - The streams created by [`accept`] and [`incoming`] behave similarly to how client-side streams behave in nonblocking mode. See the documentation for `set_nonblocking` for an explanation of the exact effects.
    ///
    /// [`accept`]: struct.PipeListener.html#method.accept
    /// [`incoming`]: struct.PipeListener.html#method.incoming
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    pub nonblocking: bool,
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
macro_rules! genset {
    ($name:ident : $ty:ty) => {
        #[doc = concat!("Sets the [`", stringify!($name), "`](#structfield.", stringify!($name), ") parameter to the specified value.")]
        #[inline]
        #[must_use = "builder setters take the entire structure and return the result"]
        pub fn $name(mut self, $name: impl Into<$ty>) -> Self {
            self.$name = $name.into();
            self
        }
    };
    ($($name:ident : $ty:ty),+ $(,)?) => {
        $(genset!($name : $ty);)+
    };
}
impl<'a> PipeListenerOptions<'a> {
    /// Creates a new builder with default options.
    #[inline]
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed(OsStr::new("")),
            mode: PipeMode::Bytes,
            nonblocking: false,
            instance_limit: None,
            write_through: false,
            accept_remote: false,
            input_buffer_size_hint: 512,
            output_buffer_size_hint: 512,
            wait_timeout: NonZeroU32::new(50).unwrap(),
        }
    }
    genset!(
        name: Cow<'a, OsStr>,
        mode: PipeMode,
        nonblocking: bool,
        instance_limit: Option<NonZeroU8>,
        write_through: bool,
        accept_remote: bool,
        input_buffer_size_hint: usize,
        output_buffer_size_hint: usize,
        wait_timeout: NonZeroU32,
    );
    /// Creates an instance of a pipe for a listener with the specified stream type and with the first-instance flag set to the specified value.
    fn create_instance<Stream: PipeStream>(
        &self,
        first: bool,
        nonblocking: bool,
    ) -> io::Result<PipeOps> {
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
                self.mode.to_pipe_type()
                    | Stream::READ_MODE.map_or(0, |x| x.to_readmode())
                    | nonblocking as u32,
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
            Ok(unsafe { PipeOps::from_raw_handle(handle) })
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
            nonblocking: self.nonblocking,
            instance_limit: self.instance_limit,
            write_through: self.write_through,
            accept_remote: self.accept_remote,
            input_buffer_size_hint: self.input_buffer_size_hint,
            output_buffer_size_hint: self.output_buffer_size_hint,
            wait_timeout: self.wait_timeout,
        };
        let instancer_capacity = self.instance_limit.map_or(8, |x| x.get()) as usize;
        let mut instance_vec = Vec::with_capacity(instancer_capacity);
        let first_instance = Arc::new((
            self.create_instance::<Stream>(true, self.nonblocking)?,
            AtomicBool::new(false),
        ));
        instance_vec.push(first_instance);
        Ok(PipeListener {
            config: owned_config,
            nonblocking: AtomicBool::new(self.nonblocking),
            instancer: Instancer(RwLock::new(instance_vec)),
            _phantom: PhantomData,
        })
    }
}
impl Default for PipeListenerOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}
