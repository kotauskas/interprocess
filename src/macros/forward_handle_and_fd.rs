//! Forwarding macros that implement safe handle manipulation in terms of a field's implementations. Usually followed up
//! by one of the derives from `derive_raw`.

macro_rules! forward_as_handle {
    (@impl $ty:ident, $hty:ident, $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            fn $mtd(&self) -> ::std::os::$cfg::io::$hty<'_> {
                ::std::os::$cfg::io::$trt::$mtd(&self.0)
            }
        }
    };
    ($ty:ident, windows) => {
        forward_as_handle!(@impl $ty, BorrowedHandle, AsHandle, as_handle, windows);
    };
    ($ty:ident, unix) => {
        forward_as_handle!(@impl $ty, BorrowedFd, AsFd, as_fd, unix);
    };
    ($ty:ident) => {
        forward_as_handle!($ty, windows);
        forward_as_handle!($ty, unix);
    };
}

macro_rules! forward_into_handle {
    (@impl $ty:ident, $hty:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::convert::From<$ty> for ::std::os::$cfg::io::$hty {
            #[inline]
            fn from(x: $ty) -> Self {
                ::std::convert::From::from(x.0)
            }
        }
    };
    ($ty:ident, windows) => {
        forward_into_handle!(@impl $ty, OwnedHandle, windows);
    };
    ($ty:ident, unix) => {
        forward_into_handle!(@impl $ty, OwnedFd, unix);
    };
    ($ty:ident) => {
        forward_into_handle!($ty, windows);
        forward_into_handle!($ty, unix);
    };
}

macro_rules! forward_from_handle {
    (@impl $ty:ident, $hty:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::convert::From<::std::os::$cfg::io::$hty> for $ty {
            #[inline]
            fn from(x: ::std::os::$cfg::io::$hty) -> Self {
                Self(::std::convert::From::from(x))
            }
        }
    };
    ($ty:ident, windows) => {
        forward_from_handle!(@impl $ty, OwnedHandle, windows);
    };
    ($ty:ident, unix) => {
        forward_from_handle!(@impl $ty, OwnedFd, unix);
    };
    ($ty:ident) => {
        forward_from_handle!($ty, windows);
        forward_from_handle!($ty, unix);
    };
}

macro_rules! forward_asinto_handle {
    ($ty:ident, windows) => {
        forward_as_handle!($ty, windows);
        forward_into_handle!($ty, windows);
    };
    ($ty:ident, unix) => {
        forward_as_handle!($ty, unix);
        forward_into_handle!($ty, unix);
    };
    ($ty:ident) => {
        forward_asinto_handle!($ty, windows);
        forward_asinto_handle!($ty, unix);
    };
}

macro_rules! forward_handle {
    ($ty:ident, windows) => {
        forward_asinto_handle!($ty, windows);
        forward_from_handle!($ty, windows);
    };
    ($ty:ident, unix) => {
        forward_asinto_handle!($ty, unix);
        forward_from_handle!($ty, unix);
    };
    ($ty:ident) => {
        forward_handle!($ty, windows);
        forward_handle!($ty, unix);
    };
}

macro_rules! forward_try_into_handle {
    (@impl $ty:ident, $fldt:path, $hty:ident, $cfg:ident) => {
        /// Releases ownership of the handle/file descriptor, detaches the object from the async runtime and returns the
        /// handle/file descriptor as an owned object.
        ///
        /// # Errors
        /// If called outside the async runtime that corresponds to this type.
        #[cfg($cfg)]
        impl ::std::convert::TryFrom<$ty> for ::std::os::$cfg::io::$hty {
            type Error = <::std::os::$cfg::io::$hty as ::std::convert::TryFrom<$fldt>>::Error;
            #[inline]
            fn try_from(x: $ty) -> Result<Self, Self::Error> {
                ::std::convert::TryFrom::try_from(x.0)
            }
        }
    };
    ($ty:ident, $fldt:path, windows) => {
        forward_try_into_handle!(@impl $ty, $fldt, OwnedHandle, windows);
    };
    ($ty:ident, $fldt:path, unix) => {
        forward_try_into_handle!(@impl $ty, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_into_handle!($ty, windows);
        forward_try_into_handle!($ty, unix);
    };
}

macro_rules! forward_try_from_handle {
    (@impl $ty:ident, $fldt:path, $hty:ident, $cfg:ident) => {
        /// Creates an async object from a given owned handle/file descriptor. This will also attach the object to the
        /// async runtime this function is called in.
        ///
        /// # Errors
        /// If called outside the async runtime that corresponds to this type.
        #[cfg($cfg)]
        impl ::std::convert::TryFrom<::std::os::$cfg::io::$hty> for $ty {
            type Error = <$fldt as ::std::convert::TryFrom<::std::os::$cfg::io::$hty>>::Error;
            #[inline]
            fn try_from(x: ::std::os::$cfg::io::$hty) -> Result<Self, Self::Error> {
                Ok(Self(::std::convert::TryFrom::try_from(x)?))
            }
        }
    };
    ($ty:ident, $fldt:path, windows) => {
        forward_try_from_handle!(@impl $ty, $fldt, OwnedHandle, windows);
    };
    ($ty:ident, $fldt:path, unix) => {
        forward_try_from_handle!(@impl $ty, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_from_handle!($ty, $fldt, windows);
        forward_try_from_handle!($ty, $fldt, unix);
    };
}

macro_rules! forward_try_handle {
    ($ty:ident, $fldt:path, windows) => {
        forward_try_into_handle!($ty, $fldt, windows);
        forward_try_from_handle!($ty, $fldt, windows);
    };
    ($ty:ident, $fldt:path, unix) => {
        forward_try_into_handle!($ty, $fldt, unix);
        forward_try_from_handle!($ty, $fldt, unix);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_handle!($ty, $fldt, windows);
        forward_try_handle!($ty, $fldt, unix);
    };
}
