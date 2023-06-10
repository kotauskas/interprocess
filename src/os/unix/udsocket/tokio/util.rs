macro_rules! tokio_wrapper_conversion_methods {
    (@tokio $tok:ty) => {
        /// Unwraps into Tokio's corresponding type. This is a zero-cost operation.
        #[inline]
        pub fn into_tokio(self) -> $tok {
            self.0
        }
        /// Wraps Tokio's corresponding type. This is a zero-cost operation.
        #[inline]
        pub fn from_tokio(tokio: $tok) -> Self {
            Self(tokio)
        }
    };
    (@sync $sync:ty) => {
        /// Detaches the async object from the Tokio runtime (therefore has to be called within the runtime) and converts it to a blocking one.
        pub fn into_sync(self) -> ::std::io::Result<$sync> {
            let fd: ::std::os::unix::io::OwnedFd = ::std::convert::TryFrom::try_from(self)?;
            Ok(::std::convert::From::from(fd))
        }
        /// Creates a Tokio-based async object from a blocking one. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error.
        pub fn from_sync(sync: $sync) -> ::std::io::Result<Self> {
            let fd: ::std::os::unix::io::OwnedFd = ::std::convert::From::from(sync);
            ::std::convert::TryFrom::try_from(fd)
        }
    };
    (@std $std:ty) => {
        /// Detaches the async object from the Tokio runtime and converts it to a blocking one from the standard library. Returns an error if called outside a Tokio runtime context.
        pub fn into_std(self) -> ::std::io::Result<$std> {
            let fd: ::std::os::unix::io::OwnedFd = ::std::convert::TryFrom::try_from(self)?;
            Ok(::std::convert::From::from(fd))
        }
        /// Creates a Tokio-based async object from a blocking one from the standard library. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error.
        pub fn from_std(std: $std) -> ::std::io::Result<Self> {
            let fd: ::std::os::unix::io::OwnedFd = ::std::convert::From::from(std);
            ::std::convert::TryFrom::try_from(fd)
        }
    };
    ($($k:ident $v:ty),+ $(,)?) => {
        $(tokio_wrapper_conversion_methods!(@$k $v);)+
    };
}

macro_rules! tokio_wrapper_trait_impls {
    (for $slf:ty, @@tokio_nofd {$($gen:tt)*} $tok:ty) => {
        impl $($gen)* From<$slf> for $tok {
            #[inline]
            fn from(x: $slf) -> Self {
                x.into_tokio()
            }
        }
        impl $($gen)* From<$tok> for $slf {
            #[inline]
            fn from(tokio: $tok) -> Self {
                Self::from_tokio(tokio)
            }
        }
    };
    (for $slf:ty, @tokio_nofd $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @@tokio_nofd {} $tok);
    };
    (for $slf:ty, @tokio_nofd_lt $lt:lifetime $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @@tokio_nofd {<$lt>} $tok);
    };
    (for $slf:ty, tokio_nofd_lt $lt:lifetime $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @tokio_nofd_lt $lt $tok);
    };
    (for $slf:ty, @@tokio_onlyasfd {$($gen:tt)*} $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @@tokio_nofd {$($gen)*} $tok);

        impl $($gen)* ::std::os::unix::io::AsFd for $slf {
            #[inline]
            fn as_fd(&self) -> ::std::os::unix::io::BorrowedFd<'_> {
                ::std::os::unix::io::AsFd::as_fd(&self.0)
            }
        }
    };
    (for $slf:ty, @tokio_onlyasfd $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @@tokio_onlyasfd {} $tok);
    };
    (for $slf:ty, @tokio_onlyasfd_lt $lt:lifetime $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @@tokio_onlyasfd {<$lt>} $tok);
    };
    (for $slf:ty, tokio_onlyasfd_lt $lt:lifetime $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @tokio_onlyasfd_lt $lt $tok);
    };
    (for $slf:ty, @tokio $tok:ty) => {
        tokio_wrapper_trait_impls!(for $slf, @tokio_onlyasfd $tok);

        /// Releases ownership of the raw file descriptor, detaches the object from the Tokio runtime and returns the file descriptor as an [`OwnedFd`](::std::os::unix::io::OwnedFd).
        ///
        /// # Errors
        /// Returns an error if called outside of a Tokio runtime.
        impl ::std::convert::TryFrom<$slf> for ::std::os::unix::io::OwnedFd {
            type Error = ::std::io::Error;
            fn try_from(x: $slf) -> ::std::io::Result<Self> {
                let std = <$tok>::into_std(x.0)?;
                let fd = ::std::convert::From::from(std);
                Ok(fd)
            }
        }
        /// Creates a Tokio-based async object from a given owned file descriptor. This will also attach the object to the Tokio runtime this function is called in, so calling it outside a runtime will result in an error.
        ///
        /// # Errors
        /// Returns an error if called outside of a Tokio runtime.
        impl ::std::convert::TryFrom<::std::os::unix::io::OwnedFd> for $slf {
            type Error = ::std::io::Error;
            fn try_from(x: ::std::os::unix::io::OwnedFd) -> ::std::io::Result<Self> {
                let std = ::std::convert::From::from(x);
                let tokio = <$tok>::from_std(std)?;
                Ok(Self(tokio))
            }
        }
    };
    (for $slf:ty, @sync $sync:ty) => {
        impl ::std::convert::TryFrom<$slf> for $sync {
            type Error = ::std::io::Error;
            #[inline]
            fn try_from(x: $slf) -> Result<Self, Self::Error> {
                x.into_sync()
            }
        }
        impl ::std::convert::TryFrom<$sync> for $slf {
            type Error = ::std::io::Error;
            #[inline]
            fn try_from(sync: $sync) -> Result<Self, Self::Error> {
                Self::from_sync(sync)
            }
        }
    };
    (for $slf:ty, @std $std:ty) => {
        impl ::std::convert::TryFrom<$slf> for $std {
            type Error = ::std::io::Error;
            fn try_from(x: $slf) -> Result<Self, Self::Error> {
                x.into_std()
            }
        }
        impl ::std::convert::TryFrom<$std> for $slf {
            type Error = ::std::io::Error;
            #[inline]
            fn try_from(std: $std) -> Result<Self, Self::Error> {
                Self::from_std(std)
            }
        }
    };
    (for $slf:ty, $($k:ident $v:ty),+ $(,)?) => {
        $(tokio_wrapper_trait_impls!(for $slf, @$k $v);)+
    };
}
