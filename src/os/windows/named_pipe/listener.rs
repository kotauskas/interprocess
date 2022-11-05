use {
    crate::os::windows::{
        imports::*,
        named_pipe::{PipeMode, PipeOps, PipeStream, PipeStreamRole},
    },
    std::{
        borrow::Cow,
        convert::TryInto,
        ffi::OsStr,
        fmt::{self, Debug, Formatter},
        io,
        marker::PhantomData,
        mem::replace,
        num::{NonZeroU32, NonZeroU8},
        ptr,
        sync::{
            atomic::{AtomicBool, Ordering::Relaxed},
            Mutex,
        },
    },
    to_method::To,
};

/// The server for a named pipe, listening for connections to clients and producing pipe streams.
///
/// The only way to create a `PipeListener` is to use [`PipeListenerOptions`]. See its documentation for more.
pub struct PipeListener<Stream: PipeStream> {
    config: PipeListenerOptions<'static>, // We need the options to create new instances
    nonblocking: AtomicBool,
    stored_instance: Mutex<PipeOps>,
    _phantom: PhantomData<fn() -> Stream>,
}
/// An iterator that infinitely [`accept`]s connections on a [`PipeListener`].
///
/// This iterator is created by the [`incoming`] method on [`PipeListener`]. See its documentation for more.
///
/// [`accept`]: struct.PipeListener.html#method.accept " "
/// [`incoming`]: struct.PipeListener.html#method.incoming " "
pub struct Incoming<'a, Stream: PipeStream> {
    listener: &'a PipeListener<Stream>,
}
impl<'a, Stream: PipeStream> Iterator for Incoming<'a, Stream> {
    type Item = io::Result<Stream>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
}
impl<'a, Stream: PipeStream> IntoIterator for &'a PipeListener<Stream> {
    type IntoIter = Incoming<'a, Stream>;
    type Item = <Incoming<'a, Stream> as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.incoming()
    }
}
impl<Stream: PipeStream> PipeListener<Stream> {
    /// Blocks until a client connects to the named pipe, creating a `Stream` to communicate with the pipe.
    ///
    /// See `incoming` for an iterator version of this.
    pub fn accept(&self) -> io::Result<Stream> {
        let instance_to_hand_out = {
            let mut stored_instance = self.stored_instance.lock().expect("unexpected lock poison");
            // Doesn't actually even need to be atomic to begin with, but it's simpler and more
            // convenient to do this instead. The mutex takes care of ordering.
            let nonblocking = self.nonblocking.load(Relaxed);
            stored_instance.connect_server()?;
            let new_instance = self.create_instance(nonblocking)?;
            replace(&mut *stored_instance, new_instance)
        };

        Ok(Stream::build(super::stream::Instance::new(
            instance_to_hand_out,
            true,
        )))
    }
    /// Creates an iterator which accepts connections from clients, blocking each time `next()` is called until one connects.
    pub fn incoming(&self) -> Incoming<'_, Stream> {
        Incoming { listener: self }
    }
    /// Enables or disables the nonblocking mode for all existing instances of the listener and future ones. By default, it is disabled.
    ///
    /// This should ideally be done during creation, using the [`nonblocking` field] of the creation options, unless there's a good reason not to. This allows making one less system call during creation.
    ///
    /// See the documentation of the aforementioned field for the exact effects of enabling this mode.
    ///
    /// [`nonblocking` field]: struct.PipeListenerOptions.html#structfield.nonblocking " "
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let instance = self.stored_instance.lock().expect("unexpected lock poison");
        // Doesn't actually even need to be atomic to begin with, but it's simpler and more
        // convenient to do this instead. The mutex takes care of ordering.
        self.nonblocking.store(nonblocking, Relaxed);
        unsafe {
            super::set_nonblocking_for_stream(
                instance.as_raw_handle(),
                Stream::READ_MODE,
                nonblocking,
            )?;
        }
        // Make it clear that the lock survives until this moment.
        drop(instance);
        Ok(())
    }

    fn create_instance(&self, nonblocking: bool) -> io::Result<PipeOps> {
        let handle = self.config.create_instance(
            false,
            nonblocking,
            false,
            Stream::ROLE,
            Stream::READ_MODE,
        )?;
        // SAFETY: we just created this handle
        Ok(unsafe { PipeOps::from_raw_handle(handle) })
    }
}
impl<Stream: PipeStream> Debug for PipeListener<Stream> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeListener")
            .field("config", &self.config)
            .field("instance", &self.stored_instance)
            .field("nonblocking", &self.nonblocking.load(Relaxed))
            .finish()
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
    // TODO change into DWORD, i.e. u32
    pub input_buffer_size_hint: usize,
    /// Specifies how big the output buffer should be. The system will automatically adjust this size to align it as required or clip it by the minimum or maximum buffer size.
    pub output_buffer_size_hint: usize,
    /// The default timeout when waiting for a client to connect. Used unless another timeout is specified when waiting for a client.
    pub wait_timeout: NonZeroU32,
}
macro_rules! genset {
    // TODO get rid of this $namel thing when bumping MSRV in 2.0.0
    ($name:ident $namel:literal : $ty:ty) => {
        #[doc = "Sets the [`"]
        #[doc = $namel]
        #[doc = "`](#structfield."]
        #[doc = $namel]
        #[doc = ") parameter to the specified value."]
        #[must_use = "builder setters take the entire structure and return the result"]
        pub fn $name(mut self, $name: impl Into<$ty>) -> Self {
            self.$name = $name.into();
            self
        }
    };
    ($($name:ident $namel:literal : $ty:ty),+ $(,)?) => {
        $(genset!($name $namel : $ty);)+
    };
}
impl<'a> PipeListenerOptions<'a> {
    /// Creates a new builder with default options.
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
    /// Clones configuration options which are not owned by value and returns a copy of the original option table which is guaranteed not to borrow anything and thus ascribes to the `'static` lifetime.
    ///
    /// This is used instead of the `ToOwned` trait for backwards compatibility – this will be fixed in the next breaking release.
    pub fn to_owned(&self) -> PipeListenerOptions<'static> {
        // We need this ugliness because the compiler does not understand that
        // PipeListenerOptions<'a> can coerce into PipeListenerOptions<'static> if we manually
        // replace the name field with Cow::Owned and just copy all other elements over thanks
        // to the fact that they don't contain a mention of the lifetime 'a. Tbh we need an
        // RFC for this, would be nice.
        PipeListenerOptions {
            name: Cow::Owned(self.name.clone().into_owned()),
            mode: self.mode,
            nonblocking: self.nonblocking,
            instance_limit: self.instance_limit,
            write_through: self.write_through,
            accept_remote: self.accept_remote,
            input_buffer_size_hint: self.input_buffer_size_hint,
            output_buffer_size_hint: self.output_buffer_size_hint,
            wait_timeout: self.wait_timeout,
        }
    }
    genset!(
        name "name": Cow<'a, OsStr>,
        mode "mode": PipeMode,
        nonblocking "nonblocking": bool,
        instance_limit "instance_limit": Option<NonZeroU8>,
        write_through "write_through": bool,
        accept_remote "accept_remote": bool,
        input_buffer_size_hint "input_buffer_size_hint": usize,
        output_buffer_size_hint "output_buffer_size_hint": usize,
        wait_timeout "wait_timeout": NonZeroU32,
    );
    /// Creates an instance of a pipe for a listener with the specified stream type and with the first-instance flag set to the specified value.
    pub(super) fn create_instance(
        &self,
        first: bool,
        nonblocking: bool,
        overlapped: bool,
        role: PipeStreamRole,
        read_mode: Option<PipeMode>,
    ) -> io::Result<HANDLE> {
        if read_mode == Some(PipeMode::Messages) && self.mode == PipeMode::Bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "\
cannot create pipe server that has byte type but reads messages – have you forgotten to set the \
`mode` field in `PipeListenerOptions`?",
            ));
        }

        let path = super::convert_path(&self.name, None);
        let open_mode = self.to_open_mode(first, role, overlapped);
        let pipe_mode = self.to_pipe_mode(read_mode, nonblocking);
        let (handle, success) = unsafe {
            // TODO security attributes
            let handle = CreateNamedPipeW(
                path.as_ptr(),
                open_mode,
                pipe_mode,
                self.instance_limit.map_or(255, |x| {
                    assert!(x.get() != 255, "cannot set 255 as the named pipe instance limit due to 255 being a reserved value");
                    x.get().to::<DWORD>()
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
            Ok(handle)
        } else {
            Err(io::Error::last_os_error())
        }
    }
    /// Creates the pipe listener from the builder. The `Stream` generic argument specifies the type of pipe stream that the listener will create, thus determining the direction of the pipe and its mode.
    ///
    /// For outbound or duplex pipes, the `mode` parameter must agree with the `Stream`'s `WRITE_MODE`. Otherwise, the call will panic in debug builds or, in release builds, the `WRITE_MODE` will take priority.
    pub fn create<Stream: PipeStream>(&self) -> io::Result<PipeListener<Stream>> {
        let (owned_config, instance) = self._create(Stream::ROLE, Stream::READ_MODE)?;
        let nonblocking = owned_config.nonblocking.into();
        Ok(PipeListener {
            config: owned_config,
            nonblocking,
            stored_instance: Mutex::new(instance),
            _phantom: PhantomData,
        })
    }
    fn _create(
        &self,
        role: PipeStreamRole,
        read_mode: Option<PipeMode>,
    ) -> io::Result<(PipeListenerOptions<'static>, PipeOps)> {
        let owned_config = self.to_owned();

        let instance = {
            let handle = self.create_instance(true, self.nonblocking, false, role, read_mode)?;
            unsafe {
                // SAFETY: we just created this handle, so we know it's unique (and we've checked
                // that it's valid)
                PipeOps::from_raw_handle(handle)
            }
        };
        Ok((owned_config, instance))
    }

    fn to_open_mode(&self, first: bool, role: PipeStreamRole, overlapped: bool) -> DWORD {
        let mut open_mode = 0_u32;
        open_mode |= role.direction_as_server().to::<DWORD>();
        if first {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        if self.write_through {
            open_mode |= FILE_FLAG_WRITE_THROUGH;
        }
        if overlapped {
            open_mode |= FILE_FLAG_OVERLAPPED;
        }
        open_mode
    }
    fn to_pipe_mode(&self, read_mode: Option<PipeMode>, nonblocking: bool) -> DWORD {
        let mut pipe_mode = 0_u32;
        pipe_mode |= self.mode.to_pipe_type();
        pipe_mode |= read_mode.map_or(0, PipeMode::to_readmode);
        if nonblocking {
            pipe_mode |= PIPE_NOWAIT;
        }
        if !self.accept_remote {
            pipe_mode |= PIPE_REJECT_REMOTE_CLIENTS;
        }
        pipe_mode
    }
}
impl Default for PipeListenerOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}
