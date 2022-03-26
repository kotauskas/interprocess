macro_rules! impl_as_raw_handle {
    ($ty:ident) => {
        #[cfg(doc)]
        impl $crate::os::windows::imports::AsRawHandle for $ty {
            #[cfg(windows)]
            fn as_raw_handle(&self) -> *mut ::std::ffi::c_void {
                $crate::os::windows::imports::AsRawHandle::as_raw_handle(&self.inner)
            }
        }
        #[cfg(all(not(doc), windows))]
        impl ::std::os::windows::io::AsRawHandle for $ty {
            fn as_raw_handle(&self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::AsRawHandle::as_raw_handle(&self.inner)
            }
        }
        #[cfg(doc)]
        impl $crate::os::unix::imports::AsRawFd for $ty {
            #[cfg(unix)]
            fn as_raw_fd(&self) -> ::libc::c_int {
                $crate::os::unix::imports::AsRawFd::as_raw_fd(&self.inner)
            }
        }
        #[cfg(all(not(doc), unix))]
        impl $crate::os::unix::imports::AsRawFd for $ty {
            fn as_raw_fd(&self) -> ::libc::c_int {
                ::std::os::unix::io::AsRawFd::as_raw_fd(&self.inner)
            }
        }
    };
}
macro_rules! impl_into_raw_handle {
    ($ty:ident) => {
        #[cfg(doc)]
        impl $crate::os::windows::imports::IntoRawHandle for $ty {
            #[cfg(windows)]
            fn into_raw_handle(self) -> *mut ::std::ffi::c_void {
                $crate::os::windows::imports::IntoRawHandle::into_raw_handle(self.inner)
            }
        }
        #[cfg(all(not(doc), windows))]
        impl ::std::os::windows::io::IntoRawHandle for $ty {
            fn into_raw_handle(self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::IntoRawHandle::into_raw_handle(self.inner)
            }
        }
        #[cfg(doc)]
        impl $crate::os::unix::imports::IntoRawFd for $ty {
            #[cfg(unix)]
            fn into_raw_fd(self) -> ::libc::c_int {
                $crate::os::unix::imports::IntoRawFd::into_raw_fd(self.inner)
            }
        }
        #[cfg(all(not(doc), unix))]
        impl ::std::os::unix::io::IntoRawFd for $ty {
            fn into_raw_fd(self) -> ::libc::c_int {
                ::std::os::unix::io::IntoRawFd::into_raw_fd(self.inner)
            }
        }
    };
}
macro_rules! impl_from_raw_handle {
    ($ty:ident) => {
        #[cfg(doc)]
        impl $crate::os::windows::imports::FromRawHandle for $ty {
            #[cfg(windows)]
            unsafe fn from_raw_handle(handle: *mut ::std::ffi::c_void) -> Self {
                Self {
                    inner: unsafe {
                        $crate::os::windows::imports::FromRawHandle::from_raw_handle(handle)
                    },
                }
            }
        }
        #[cfg(all(not(doc), windows))]
        impl ::std::os::windows::io::FromRawHandle for $ty {
            unsafe fn from_raw_handle(handle: *mut ::std::ffi::c_void) -> Self {
                Self {
                    inner: unsafe {
                        ::std::os::windows::io::FromRawHandle::from_raw_handle(handle)
                    },
                }
            }
        }
        #[cfg(doc)]
        impl $crate::os::unix::imports::FromRawFd for $ty {
            #[cfg(unix)]
            unsafe fn from_raw_fd(fd: ::libc::c_int) -> Self {
                Self {
                    inner: unsafe { $crate::os::unix::imports::FromRawFd::from_raw_fd(fd) },
                }
            }
        }
        #[cfg(all(not(doc), unix))]
        impl ::std::os::unix::io::FromRawFd for $ty {
            unsafe fn from_raw_fd(fd: ::libc::c_int) -> Self {
                Self {
                    inner: unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) },
                }
            }
        }
    };
}
macro_rules! impl_handle_manip {
    ($ty:ident) => {
        impl_as_raw_handle!($ty);
        impl_into_raw_handle!($ty);
        impl_from_raw_handle!($ty);
    };
}
