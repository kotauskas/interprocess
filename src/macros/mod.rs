#![allow(unused_macros)]

macro_rules! impmod {
    ($($osmod:ident)::+, $($orig:ident $(as $into:ident)?),* $(,)?) => {
        #[cfg(unix)]
        use $crate::os::unix::$($osmod)::+::{$($orig $(as $into)?,)*};
        #[cfg(windows)]
        use $crate::os::windows::$($osmod)::+::{$($orig $(as $into)?,)*};
    };
}

macro_rules! multimacro {
    ($tok:tt, $($macro:ident $(($($arg:tt)+))?),+ $(,)?) => {$(
        $macro!($tok $(, $($arg)+)?);
    )+};
}

macro_rules! make_macro_modules {
    ($($modname:ident),+ $(,)?) => {$(
        #[macro_use] mod $modname;
    )+};
}

make_macro_modules! {
    ok_or_ret_errno, derive_raw,
    forward_handle_and_fd, forward_try_clone, forward_trait_method, forward_iorw, forward_fmt,
}
