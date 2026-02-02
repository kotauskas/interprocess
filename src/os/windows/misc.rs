#[allow(unused_imports)]
pub(crate) mod winprelude {
    pub(in super::super) use super::super::linger_pool;
    pub(crate) use {
        super::{
            AsRawHandleExt as _, FromRawHandleExt as _, HANDLEExt as _, IntoRawHandleExt as _,
        },
        std::os::windows::prelude::*,
        windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    };
}

use {
    crate::RawOsErrorExt as _,
    std::{
        ffi::c_void,
        io::{self, ErrorKind::BrokenPipe},
    },
    winprelude::*,
};

pub(crate) trait AsRawHandleExt: AsRawHandle {
    #[inline(always)]
    #[allow(clippy::as_conversions)]
    fn as_int_handle(&self) -> HANDLE { self.as_raw_handle() as HANDLE }
}
impl<T: AsRawHandle + ?Sized> AsRawHandleExt for T {}
pub(crate) trait IntoRawHandleExt: IntoRawHandle + Sized {
    #[inline(always)]
    #[allow(clippy::as_conversions)]
    fn into_int_handle(self) -> HANDLE { self.into_raw_handle() as HANDLE }
}
impl<T: IntoRawHandle> IntoRawHandleExt for T {}
pub(crate) trait FromRawHandleExt: FromRawHandle + Sized {
    #[inline(always)]
    #[allow(clippy::as_conversions)]
    unsafe fn from_int_handle(h: HANDLE) -> Self {
        // FUTURE use null provenance instead of int2ptr
        unsafe { Self::from_raw_handle(h as *mut c_void) }
    }
}
impl<T: FromRawHandle> FromRawHandleExt for T {}

pub(crate) trait HANDLEExt {
    fn to_std(self) -> RawHandle;
}
impl HANDLEExt for HANDLE {
    #[inline(always)]
    #[allow(clippy::as_conversions)]
    fn to_std(self) -> RawHandle { self as RawHandle }
}

pub(super) fn decode_eof<T>(r: io::Result<T>) -> io::Result<T> {
    use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
    match r {
        Err(e) if e.raw_os_error().eeq(ERROR_PIPE_NOT_CONNECTED) => {
            Err(io::Error::from(BrokenPipe))
        }
        els => els,
    }
}
pub(super) fn downgrade_eof<T: Default>(r: io::Result<T>) -> io::Result<T> {
    match decode_eof(r) {
        Err(e) if e.kind() == BrokenPipe => Ok(T::default()),
        els => els,
    }
}
