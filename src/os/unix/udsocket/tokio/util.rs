macro_rules! tokio_wrapper_conversion_methods {
    (tokio_norawfd $tok:ty) => {
        /// Unwraps into Tokio's corresponding type. This is a zero-cost operation.
        pub fn into_tokio(self) -> $tok {
            self.0
        }
        /// Wraps Tokio's corresponding type. This is a zero-cost operation.
        pub fn from_tokio(tokio: $tok) -> Self {
            Self(tokio)
        }
    };
    (tokio $tok:ty) => {
        tokio_wrapper_conversion_methods!(tokio_norawfd $tok);
        /// Creates a Tokio-based async object from a given raw file descriptor. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error (which is why the `FromRawFd` trait can't be implemented instead).
        ///
        /// # Safety
        /// The given file descriptor must be valid (i.e. refer to an existing kernel object) and must not be owned by any other file descriptor container. If this is not upheld, an arbitrary file descriptor will be closed when the returned object is dropped.
        pub unsafe fn from_raw_fd(fd: c_int) -> io::Result<Self> {
            let std = unsafe { FromRawFd::from_raw_fd(fd) };
            let tokio = <$tok>::from_std(std)?;
            Ok(Self(tokio))
        }
        /// Releases ownership of the raw file descriptor, detaches the object from the Tokio runtime (therefore has to be called within the runtime) and returns the file descriptor as an integer.
        pub fn into_raw_fd(self) -> io::Result<c_int> {
            let std = <$tok>::into_std(self.0)?;
            let fd = IntoRawFd::into_raw_fd(std);
            Ok(fd)
        }
    };
    (sync $sync:ty) => {
        /// Detaches the async object from the Tokio runtime (therefore has to be called within the runtime) and converts it to a blocking one.
        pub fn into_sync(self) -> io::Result<$sync> {
            Ok(unsafe { <$sync as FromRawFd>::from_raw_fd(self.into_raw_fd()?) })
        }
        /// Creates a Tokio-based async object from a blocking one. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error.
        pub fn from_sync(sync: $sync) -> io::Result<Self> {
            let fd = IntoRawFd::into_raw_fd(sync);
            unsafe { Self::from_raw_fd(fd) }
        }
    };
    (std $std:ty) => {
        /// Detaches the async object from the Tokio runtime and converts it to a blocking one from the standard library. Returns an error if called outside a Tokio runtime context.
        pub fn into_std(self) -> io::Result<$std> {
            Ok(unsafe { <$std as FromRawFd>::from_raw_fd(self.into_raw_fd()?) })
        }
        /// Creates a Tokio-based async object from a blocking one from the standard library. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error.
        pub fn from_std(std: $std) -> io::Result<Self> {
            let fd = IntoRawFd::into_raw_fd(std);
            unsafe { Self::from_raw_fd(fd) }
        }
    };
    ($($k:ident $v:ty),+ $(,)?) => {
        $(tokio_wrapper_conversion_methods!($k $v);)+
    };
}

macro_rules! tokio_wrapper_trait_impls {
    (for $slf:ty, @tokio_norawfd {$($gen:tt)*} $tok:ty) => {
        impl $($gen)* From<$slf> for $tok {
            fn from(x: $slf) -> Self {
                x.into_tokio()
            }
        }
        impl $($gen)* From<$tok> for $slf {
            fn from(tokio: $tok) -> Self {
                Self::from_tokio(tokio)
            }
        }
    };
    (for $slf:ty, tokio_norawfd $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @tokio_norawfd {} $tok);
    };
    (for $slf:ty, tokio_norawfd_lt $lt:lifetime $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @tokio_norawfd {<$lt>} $tok);
    };
    (for $slf:ty, tokio $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, tokio_norawfd $tok);

        impl AsRawFd for $slf {
            #[cfg(unix)]
            fn as_raw_fd(&self) -> c_int {
                self.0.as_raw_fd()
            }
        }
    };
    (for $slf:ty, sync $sync:ty) => {
        impl TryFrom<$slf> for $sync {
            type Error = io::Error;
            fn try_from(x: $slf) -> Result<Self, Self::Error> {
                x.into_sync()
            }
        }
        impl TryFrom<$sync> for $slf {
            type Error = io::Error;
            fn try_from(sync: $sync) -> Result<Self, Self::Error> {
                Self::from_sync(sync)
            }
        }
    };
    (for $slf:ty, std $std:ty) => {
        impl TryFrom<$slf> for $std {
            type Error = io::Error;
            fn try_from(x: $slf) -> Result<Self, Self::Error> {
                x.into_std()
            }
        }
        impl TryFrom<$std> for $slf {
            type Error = io::Error;
            fn try_from(std: $std) -> Result<Self, Self::Error> {
                Self::from_std(std)
            }
        }
    };
    (for $slf:ty, $($k:ident $v:ty),+ $(,)?) => {
        $(tokio_wrapper_trait_impls!(for $slf, $k $v);)+
    };
}
