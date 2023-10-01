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
    (windows: $ty:ident) => {
        forward_as_handle!(@impl $ty, BorrowedHandle, AsHandle, as_handle, windows);
    };
    (unix: $ty:ident) => {
        forward_as_handle!(@impl $ty, BorrowedFd, AsFd, as_fd, unix);
    };
    ($ty:ident) => {
        forward_as_handle!(windows: $ty);
        forward_as_handle!(unix: $ty);
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
    (windows: $ty:ident) => {
        forward_into_handle!(@impl $ty, OwnedHandle, windows);
    };
    (unix: $ty:ident) => {
        forward_into_handle!(@impl $ty, OwnedFd, unix);
    };
    ($ty:ident) => {
        forward_into_handle!(windows: $ty);
        forward_into_handle!(unix: $ty);
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
    (windows: $ty:ident) => {
        forward_from_handle!(@impl $ty, OwnedHandle, windows);
    };
    (unix: $ty:ident) => {
        forward_from_handle!(@impl $ty, OwnedFd, unix);
    };
    ($ty:ident) => {
        forward_from_handle!(windows: $ty);
        forward_from_handle!(unix: $ty);
    };
}

macro_rules! forward_asinto_handle {
    (windows: $ty:ident) => {
        forward_as_handle!(windows: $ty);
        forward_into_handle!(windows: $ty);
    };
    (unix: $ty:ident) => {
        forward_as_handle!(unix: $ty);
        forward_into_handle!(unix: $ty);
    };
    ($ty:ident) => {
        forward_asinto_handle!(windows: $ty);
        forward_asinto_handle!(unix: $ty);
    };
}

macro_rules! forward_handle {
    (windows: $ty:ident) => {
        forward_asinto_handle!(windows: $ty);
        forward_from_handle!(windows: $ty);
    };
    (unix: $ty:ident) => {
        forward_asinto_handle!(unix: $ty);
        forward_from_handle!(unix: $ty);
    };
    ($ty:ident) => {
        forward_handle!(windows: $ty);
        forward_handle!(unix: $ty);
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
    (windows: $ty:ident, $fldt:path) => {
        forward_try_into_handle!(@impl $ty, $fldt, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fldt:path) => {
        forward_try_into_handle!(@impl $ty, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_into_handle!(windows: $ty);
        forward_try_into_handle!(unix: $ty);
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
    (windows: $ty:ident, $fldt:path) => {
        forward_try_from_handle!(@impl $ty, $fldt, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fldt:path) => {
        forward_try_from_handle!(@impl $ty, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_from_handle!(windows: $ty, $fldt);
        forward_try_from_handle!(unix: $ty, $fldt);
    };
}

macro_rules! forward_try_handle {
    (windows: $ty:ident, $fldt:path) => {
        forward_try_into_handle!(windows: $ty, $fldt);
        forward_try_from_handle!(windows: $ty, $fldt);
    };
    (unix: $ty:ident, $fldt:path) => {
        forward_try_into_handle!(unix: $ty, $fldt);
        forward_try_from_handle!(unix: $ty, $fldt);
    };
    ($ty:ident, $fldt:path) => {
        forward_try_handle!(windows: $ty, $fldt);
        forward_try_handle!(unix: $ty, $fldt);
    };
}
