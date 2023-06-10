#![allow(unused_macros)]

// Derive macros that implement raw handle manipulation in terms of safe handle manipulation from Rust 1.63+.

macro_rules! derive_asraw {
    (@impl $ty:ident $(<$lt:lifetime>)?, $hty:ident, $trt:ident, $mtd:ident, $strt:ident, $smtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::os::$cfg::io::$trt for $ty $(<$lt>)? {
            #[inline]
            fn $mtd(&self) -> ::std::os::$cfg::io::$hty {
                let h = ::std::os::$cfg::io::$strt::$smtd(self);
                ::std::os::$cfg::io::$trt::$mtd(&h)
            }
        }
    };
    (windows: $ty:ident $(<$lt:lifetime>)?) => {
        derive_asraw!(@impl $ty $(<$lt>)?, RawHandle, AsRawHandle, as_raw_handle, AsHandle, as_handle, windows);
    };
    (unix: $ty:ident $(<$lt:lifetime>)?) => {
        derive_asraw!(@impl $ty $(<$lt>)?, RawFd, AsRawFd, as_raw_fd, AsFd, as_fd, unix);
    };
    ($ty:ident) => {
        derive_asraw!(windows: $ty);
        derive_asraw!(unix: $ty);
    };
}

macro_rules! derive_intoraw {
    (@impl $ty:ident, $hty:ident, $ohty:ident, $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            fn $mtd(self) -> ::std::os::$cfg::io::$hty {
                let h = <std::os::$cfg::io::$ohty as ::std::convert::From<_>>::from(self);
                ::std::os::$cfg::io::$trt::$mtd(h)
            }
        }
    };
    (windows: $ty:ident) => {
        derive_intoraw!(@impl $ty, RawHandle, OwnedHandle, IntoRawHandle, into_raw_handle, windows);
    };
    (unix: $ty:ident) => {
        derive_intoraw!(@impl $ty, RawFd, OwnedFd, IntoRawFd, into_raw_fd, unix);
    };
    ($ty:ident) => {
        derive_intoraw!(windows: $ty);
        derive_intoraw!(unix: $ty);
    };
}

macro_rules! derive_asintoraw {
    (windows: $ty:ident) => {
        derive_asraw!(windows: $ty);
        derive_intoraw!(windows: $ty);
    };
    (unix: $ty:ident) => {
        derive_asraw!(unix: $ty);
        derive_intoraw!(unix: $ty);
    };
    ($ty:ident) => {
        derive_asintoraw!(windows: $ty);
        derive_asintoraw!(unix: $ty);
    };
}

macro_rules! derive_fromraw {
    (@impl $ty:ident, $hty:ident, $ohty:ident, $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            unsafe fn $mtd(fd: ::std::os::$cfg::io::$hty) -> Self {
                let h: ::std::os::$cfg::io::$ohty = unsafe { ::std::os::$cfg::io::$trt::$mtd(fd) };
                ::std::convert::From::from(h)
            }
        }
    };
    (windows: $ty:ident) => {
        derive_fromraw!(@impl $ty, RawHandle, OwnedHandle, FromRawHandle, from_raw_handle, windows);
    };
    (unix: $ty:ident) => {
        derive_fromraw!(@impl $ty, RawFd, OwnedFd, FromRawFd, from_raw_fd, unix);
    };
    ($ty:ident) => {
        derive_fromraw!(windows: $ty);
        derive_fromraw!(unix: $ty);
    };
}

macro_rules! derive_raw {
    (windows: $ty:ident) => {
        derive_asintoraw!(windows: $ty);
        derive_fromraw!(windows: $ty);
    };
    (unix: $ty:ident) => {
        derive_asintoraw!(unix: $ty);
        derive_fromraw!(unix: $ty);
    };
    ($ty:ident) => {
        derive_asintoraw!($ty);
        derive_fromraw!($ty);
    };
}

// Forwarding macros that implement safe handle manipulation in terms of a field's implementations. Usually followed up
// by one of the above derives.

macro_rules! forward_as_handle {
    (@impl $ty:ident, $fld:ident, $hty:ident, $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            fn $mtd(&self) -> ::std::os::$cfg::io::$hty<'_> {
                ::std::os::$cfg::io::$trt::$mtd(&self.$fld)
            }
        }
    };
    (windows: $ty:ident, $fld:ident) => {
        forward_as_handle!(@impl $ty, $fld, BorrowedHandle, AsHandle, as_handle, windows);
    };
    (unix: $ty:ident, $fld:ident) => {
        forward_as_handle!(@impl $ty, $fld, BorrowedFd, AsFd, as_fd, unix);
    };
    ($ty:ident, $fld:ident) => {
        forward_as_handle!(windows: $ty, $fld);
        forward_as_handle!(unix: $ty, $fld);
    };
}

macro_rules! forward_into_handle {
    (@impl $ty:ident, $fld:ident, $hty:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::convert::From<$ty> for ::std::os::$cfg::io::$hty {
            #[inline]
            fn from(x: $ty) -> Self {
                ::std::convert::From::from(x.$fld)
            }
        }
    };
    (windows: $ty:ident, $fld:ident) => {
        forward_into_handle!(@impl $ty, $fld, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fld:ident) => {
        forward_into_handle!(@impl $ty, $fld, OwnedFd, unix);
    };
    ($ty:ident, $fld:ident) => {
        forward_into_handle!(windows: $ty, $fld);
        forward_into_handle!(unix: $ty, $fld);
    };
}

macro_rules! forward_from_handle {
    (@impl $ty:ident, $fld:ident, $hty:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl ::std::convert::From<::std::os::$cfg::io::$hty> for $ty {
            #[inline]
            fn from(x: ::std::os::$cfg::io::$hty) -> Self {
                Self {
                    $fld: ::std::convert::From::from(x),
                }
            }
        }
    };
    (windows: $ty:ident, $fld:ident) => {
        forward_from_handle!(@impl $ty, $fld, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fld:ident) => {
        forward_from_handle!(@impl $ty, $fld, OwnedFd, unix);
    };
    ($ty:ident, $fld:ident) => {
        forward_from_handle!(windows: $ty, $fld);
        forward_from_handle!(unix: $ty, $fld);
    };
}

macro_rules! forward_handle {
    (windows: $ty:ident, $fld:ident) => {
        forward_as_handle!(windows: $ty, $fld);
        forward_into_handle!(windows: $ty, $fld);
        forward_from_handle!(windows: $ty, $fld);
    };
    (unix: $ty:ident, $fld:ident) => {
        forward_as_handle!(unix: $ty, $fld);
        forward_into_handle!(unix: $ty, $fld);
        forward_from_handle!(unix: $ty, $fld);
    };
    ($ty:ident, $fld:ident) => {
        forward_handle!(windows: $ty, $fld);
        forward_handle!(unix: $ty, $fld);
    };
}

macro_rules! forward_try_into_handle {
    (@impl $ty:ident, $fld:ident, $fldt:path, $hty:ident, $cfg:ident) => {
        /// Releases ownership of the handle/file descriptor, detaches the object from the async runtime and returns the handle/file descriptor as an owned object.
        ///
        /// # Errors
        /// If called outside the async runtime that corresponds to this type.
        #[cfg($cfg)]
        impl ::std::convert::TryFrom<$ty> for ::std::os::$cfg::io::$hty {
            type Error = <$fldt as ::std::convert::TryFrom<::std::os::$cfg::io::$hty>>::Error;
            #[inline]
            fn try_from(x: $ty) -> Result<Self, Self::Error> {
                ::std::convert::TryFrom::try_from(x.$fld)
            }
        }
    };
    (windows: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_into_handle!(@impl $ty, $fld, $fldt, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_into_handle!(@impl $ty, $fld, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fld:ident, $fldt:path) => {
        forward_try_into_handle!(windows: $ty, $fld, $fldt);
        forward_try_into_handle!(unix: $ty, $fld, $fldt);
    };
}

macro_rules! forward_try_from_handle {
    (@impl $ty:ident, $fld:ident, $fldt:path, $hty:ident, $cfg:ident) => {
        /// Creates an async object from a given owned handle/file descriptor. This will also attach the object to the async runtime this function is called in.
        ///
        /// # Errors
        /// If called outside the async runtime that corresponds to this type.
        #[cfg($cfg)]
        impl ::std::convert::TryFrom<::std::os::$cfg::io::$hty> for $ty {
            type Error = <$fldt as ::std::convert::TryFrom<::std::os::$cfg::io::$hty>>::Error;
            #[inline]
            fn try_from(x: ::std::os::$cfg::io::$hty) -> Result<Self, Self::Error> {
                Ok(Self {
                    $fld: ::std::convert::TryFrom::try_from(x)?,
                })
            }
        }
    };
    (windows: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_from_handle!(@impl $ty, $fld, $fldt, OwnedHandle, windows);
    };
    (unix: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_from_handle!(@impl $ty, $fld, $fldt, OwnedFd, unix);
    };
    ($ty:ident, $fld:ident, $fldt:path) => {
        forward_try_from_handle!(windows: $ty, $fld, $fldt);
        forward_try_from_handle!(unix: $ty, $fld, $fldt);
    };
}

macro_rules! forward_try_handle {
    (windows: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_into_handle!(windows: $ty, $fld, $fldt);
        forward_try_from_handle!(windows: $ty, $fld, $fldt);
    };
    (unix: $ty:ident, $fld:ident, $fldt:path) => {
        forward_try_into_handle!(unix: $ty, $fld, $fldt);
        forward_try_from_handle!(unix: $ty, $fld, $fldt);
    };
    ($ty:ident, $fld:ident, $fldt:path) => {
        forward_try_handle!(windows: $ty, $fld, $fldt);
        forward_try_handle!(unix: $ty, $fld, $fldt);
    };
}

// Legacy macros.
/*
macro_rules! impl_as_raw_handle_windows {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::AsRawHandle for $ty {
            fn as_raw_handle(&self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::AsRawHandle::as_raw_handle(&self.inner)
            }
        }
    };
}
macro_rules! impl_as_raw_handle_unix {
    ($ty:ident) => {
        #[cfg(unix)]
        impl ::std::os::unix::io::AsRawFd for $ty {
            fn as_raw_fd(&self) -> ::libc::c_int {
                ::std::os::unix::io::AsRawFd::as_raw_fd(&self.inner)
            }
        }
    };
}
macro_rules! impl_as_raw_handle {
    ($ty:ident) => {
        impl_as_raw_handle_windows!($ty);
        impl_as_raw_handle_unix!($ty);
    };
}

macro_rules! impl_into_raw_handle_windows {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::IntoRawHandle for $ty {
            fn into_raw_handle(self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::IntoRawHandle::into_raw_handle(self.inner)
            }
        }
    };
}
macro_rules! impl_into_raw_handle_unix {
    ($ty:ident) => {
        #[cfg(unix)]
        impl ::std::os::unix::io::IntoRawFd for $ty {
            fn into_raw_fd(self) -> ::libc::c_int {
                ::std::os::unix::io::IntoRawFd::into_raw_fd(self.inner)
            }
        }
    };
}
macro_rules! impl_into_raw_handle {
    ($ty:ident) => {
        impl_into_raw_handle_windows!($ty);
        impl_into_raw_handle_unix!($ty);
    };
}

macro_rules! impl_from_raw_handle_windows {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::FromRawHandle for $ty {
            unsafe fn from_raw_handle(handle: *mut ::std::ffi::c_void) -> Self {
                Self {
                    inner: unsafe { ::std::os::windows::io::FromRawHandle::from_raw_handle(handle) },
                }
            }
        }
    };
}
macro_rules! impl_from_raw_handle_unix {
    ($ty:ident) => {
        #[cfg(unix)]
        impl ::std::os::unix::io::FromRawFd for $ty {
            unsafe fn from_raw_fd(fd: ::libc::c_int) -> Self {
                Self {
                    inner: unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) },
                }
            }
        }
    };
}
macro_rules! impl_from_raw_handle {
    ($ty:ident) => {
        impl_from_raw_handle_windows!($ty);
        impl_from_raw_handle_unix!($ty);
    };
}

macro_rules! impl_handle_manip_unix {
    ($ty:ident) => {
        impl_as_raw_handle_unix!($ty);
        impl_into_raw_handle_unix!($ty);
        impl_from_raw_handle_unix!($ty);
    };
}
macro_rules! impl_handle_manip_windows {
    ($ty:ident) => {
        impl_as_raw_handle_windows!($ty);
        impl_into_raw_handle_windows!($ty);
        impl_from_raw_handle_windows!($ty);
    };
}
macro_rules! impl_handle_manip {
    ($ty:ident) => {
        impl_handle_manip_unix!($ty);
        impl_handle_manip_windows!($ty);
    };
}
*/
