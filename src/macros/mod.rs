#[macro_use]
mod handle_and_fd;
#[macro_use]
mod import_tables;

macro_rules! impmod {
    ($($osmod:ident)::+, $($orig:ident $(as $into:ident)?),* $(,)?) => {
        #[cfg(unix)]
        use $crate::os::unix::$($osmod)::+::{$($orig $(as $into)?,)*};
        #[cfg(windows)]
        use $crate::os::windows::$($osmod)::+::{$($orig $(as $into)?,)*};
    };
}
