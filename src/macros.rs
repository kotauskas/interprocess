#![allow(unused_macros)]

macro_rules! impmod {
    ($($osmod:ident)::+, $($orig:ident $(as $into:ident)?),* $(,)?) => {
        #[cfg(unix)]
        use $crate::os::unix::$($osmod)::+::{$($orig $(as $into)?,)*};
        #[cfg(windows)]
        use $crate::os::windows::$($osmod)::+::{$($orig $(as $into)?,)*};
    };
}

macro_rules! ok_or_errno {
    ($success:expr => $($scb:tt)+) => {
        if $success {
            Ok($($scb)+)
        } else {
            Err(::std::io::Error::last_os_error())
        }
    };
}

macro_rules! pinproj_for_unpin {
    ($src:ty, $dst:ty) => {
        impl $src {
            #[inline(always)]
            fn pinproj(&mut self) -> ::std::pin::Pin<&mut $dst> {
                ::std::pin::Pin::new(&mut self.0)
            }
        }
    };
}

macro_rules! multimacro {
    ($pre:tt $ty:ty, $($macro:ident $(($($arg:tt)+))?),+ $(,)?) => {$(
        $macro!($pre $ty $(, $($arg)+)?);
    )+};
    ($ty:ty, $($macro:ident $(($($arg:tt)+))?),+ $(,)?) => {$(
        $macro!($ty $(, $($arg)+)?);
    )+};
}

macro_rules! make_macro_modules {
    ($($modname:ident),+ $(,)?) => {$(
        #[macro_use] mod $modname;
        #[allow(unused_imports)]
        pub(crate) use $modname::*;
    )+};
}

macro_rules! forward_rbv {
    (@$slf:ident, &) => { &$slf.0 };
    (@$slf:ident, *) => { &&*$slf.0 };
    ($ty:ty, $int:ty, $kind:tt) => {
        impl $ty {
            #[inline(always)]
            fn refwd(&self) -> &$int {
                forward_rbv!(@self, $kind)
            }
        }
    };
}

make_macro_modules! {
    derive_raw, derive_mut_iorw, derive_trivconv,
    forward_handle_and_fd, forward_try_clone, forward_trait_method, forward_iorw, forward_fmt,
}
