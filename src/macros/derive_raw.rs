//! Derive macros that implement raw handle manipulation in terms of safe handle manipulation from Rust 1.63+.

macro_rules! derive_asraw {
    (@impl
        $({$($forcl:tt)*})?
        $ty:ty,
        $hty:ident, $trt:ident, $mtd:ident,
        $strt:ident, $smtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl $(<$($forcl)*>)? ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            fn $mtd(&self) -> ::std::os::$cfg::io::$hty {
                let h = ::std::os::$cfg::io::$strt::$smtd(self);
                ::std::os::$cfg::io::$trt::$mtd(&h)
            }
        }
    };
    (windows: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawHandle, AsRawHandle, as_raw_handle,
            AsHandle, as_handle, windows);
    };
    (unix: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawFd, AsRawFd, as_raw_fd,
            AsFd, as_fd, unix);
    };
    ($({$($forcl:tt)*})? $ty:ty) => {
        derive_asraw!(windows: $({$($forcl)*})? $ty);
        derive_asraw!(unix: $({$($forcl)*})? $ty);
    };
}

macro_rules! derive_intoraw {
    (@impl
        $({$($forcl:tt)*})?
        $ty:ty,
        $hty:ident, $ohty:ident,
        $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl $(<$($forcl)*>)? ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            fn $mtd(self) -> ::std::os::$cfg::io::$hty {
                let h = <std::os::$cfg::io::$ohty as ::std::convert::From<_>>::from(self);
                ::std::os::$cfg::io::$trt::$mtd(h)
            }
        }
    };
    (windows: $({$($forcl:tt)*})? $ty:ty) => {
        derive_intoraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawHandle, OwnedHandle,
            IntoRawHandle, into_raw_handle, windows);
    };
    (unix: $({$($forcl:tt)*})? $ty:ty) => {
        derive_intoraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawFd, OwnedFd,
            IntoRawFd, into_raw_fd, unix);
    };
    ($({$($forcl:tt)*})? $ty:ty) => {
        derive_intoraw!(windows: $({$($forcl)*})? $ty);
        derive_intoraw!(unix: $({$($forcl)*})? $ty);
    };
}

macro_rules! derive_asintoraw {
    (windows: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asraw!(windows: $({$($forcl)*})? $ty);
        derive_intoraw!(windows: $({$($forcl)*})? $ty);
    };
    (unix: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asraw!(unix: $({$($forcl)*})? $ty);
        derive_intoraw!(unix: $({$($forcl)*})? $ty);
    };
    ($({$($forcl:tt)*})? $ty:ty) => {
        derive_asintoraw!(windows: $({$($forcl)*})? $ty);
        derive_asintoraw!(unix: $({$($forcl)*})? $ty);
    };
}

macro_rules! derive_fromraw {
    (@impl
        $({$($forcl:tt)*})?
        $ty:ty,
        $hty:ident, $ohty:ident,
        $trt:ident, $mtd:ident, $cfg:ident) => {
        #[cfg($cfg)]
        impl $(<$($forcl)*>)? ::std::os::$cfg::io::$trt for $ty {
            #[inline]
            unsafe fn $mtd(fd: ::std::os::$cfg::io::$hty) -> Self {
                let h: ::std::os::$cfg::io::$ohty = unsafe { ::std::os::$cfg::io::$trt::$mtd(fd) };
                ::std::convert::From::from(h)
            }
        }
    };
    (windows: $({$($forcl:tt)*})? $ty:ty) => {
        derive_fromraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawHandle, OwnedHandle,
            FromRawHandle, from_raw_handle, windows);
    };
    (unix: $({$($forcl:tt)*})? $ty:ty) => {
        derive_fromraw!(
            @impl
            $({$($forcl)*})? $ty,
            RawFd, OwnedFd,
            FromRawFd, from_raw_fd, unix);
    };
    ($({$($forcl:tt)*})? $ty:ty) => {
        derive_fromraw!(windows: $({$($forcl)*})? $ty);
        derive_fromraw!(unix: $({$($forcl)*})? $ty);
    };
}

macro_rules! derive_raw {
    (windows: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asintoraw!(windows: $({$($forcl)*})? $ty);
        derive_fromraw!(windows: $({$($forcl)*})? $ty);
    };
    (unix: $({$($forcl:tt)*})? $ty:ty) => {
        derive_asintoraw!(unix: $({$($forcl)*})? $ty);
        derive_fromraw!(unix: $({$($forcl)*})? $ty);
    };
    ($({$($forcl:tt)*})? $ty:ty) => {
        derive_asintoraw!($({$($forcl)*})? $ty);
        derive_fromraw!($({$($forcl)*})? $ty);
    };
}
