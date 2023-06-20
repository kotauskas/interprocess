#![allow(unused_macros)]

#[macro_use]
mod ok_or_ret_errno;
#[macro_use]
mod derive_raw;
#[macro_use]
mod forward_handle_and_fd;
#[macro_use]
mod forward_try_clone;

macro_rules! impmod {
    ($($osmod:ident)::+, $($orig:ident $(as $into:ident)?),* $(,)?) => {
        #[cfg(unix)]
        use $crate::os::unix::$($osmod)::+::{$($orig $(as $into)?,)*};
        #[cfg(windows)]
        use $crate::os::windows::$($osmod)::+::{$($orig $(as $into)?,)*};
    };
}
