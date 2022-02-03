macro_rules! impl_as_raw_handle {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::AsRawHandle for $ty {
            fn as_raw_handle(&self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::AsRawHandle::as_raw_handle(&self.inner)
            }
        }
        #[cfg(unix)]
        impl ::std::os::unix::io::AsRawFd for $ty {
            fn as_raw_fd(&self) -> ::libc::c_int {
                ::std::os::unix::io::AsRawFd::as_raw_fd(&self.inner)
            }
        }
    };
}
macro_rules! impl_into_raw_handle {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::IntoRawHandle for $ty {
            fn into_raw_handle(self) -> *mut ::std::ffi::c_void {
                ::std::os::windows::io::IntoRawHandle::into_raw_handle(self.inner)
            }
        }
        #[cfg(unix)]
        impl ::std::os::unix::io::IntoRawFd for $ty {
            fn into_raw_fd(self) -> ::libc::c_int {
                ::std::os::unix::io::IntoRawFd::into_raw_fd(self.inner)
            }
        }
    };
}
macro_rules! impl_from_raw_handle {
    ($ty:ident) => {
        #[cfg(windows)]
        impl ::std::os::windows::io::FromRawHandle for $ty {
            unsafe fn from_raw_handle(handle: *mut ::std::ffi::c_void) -> Self {
                Self {
                    inner: unsafe {
                        ::std::os::windows::io::FromRawHandle::from_raw_handle(handle)
                    },
                }
            }
        }
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
macro_rules! impl_handle_manip {
    ($ty:ident) => {
        impl_as_raw_handle!($ty);
        impl_into_raw_handle!($ty);
        impl_from_raw_handle!($ty);
    };
}
