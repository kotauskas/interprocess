use super::imports::*;
use cfg_if::cfg_if;
use std::{
    ffi::c_void,
    io,
    mem::{size_of, size_of_val, zeroed},
    ptr::null,
};
use to_method::To;

cfg_if! {
    if #[cfg(any(
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
        all(target_os = "linux", target_env = "musl"),
        target_env = "newlib",
        all(target_env = "uclibc", target_arch = "arm"),
        target_os = "android",
        target_os = "emscripten",
    ))] {
        pub type MsghdrSize = socklen_t;
    } else {
        pub type MsghdrSize = size_t;
    }
}

pub fn to_msghdrsize(size: usize) -> io::Result<MsghdrSize> {
    size.try_to::<MsghdrSize>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "size overflowed `socklen_t`"))
}

#[allow(unused_variables)]
pub unsafe fn enable_passcred(socket: i32) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let passcred: c_int = 1;
        unsafe {
            libc::setsockopt(
                socket,
                SOL_SOCKET,
                SO_PASSCRED,
                &passcred as *const _ as *const _,
                size_of_val(&passcred).try_to::<u32>().unwrap(),
            ) != -1
        }
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        true
    } // Cannot have passcred on macOS and iOS.
}
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub unsafe fn get_peer_ucred(socket: i32) -> io::Result<ucred> {
    let mut cred: ucred = unsafe {
        // SAFETY: it's safe for the ucred structure to be zero-initialized, since
        // it only contains integers
        zeroed()
    };
    let mut cred_len = size_of::<ucred>() as socklen_t;
    let success = unsafe {
        libc::getsockopt(
            socket,
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut cred_len as *mut _,
        )
    } != -1;
    if success {
        Ok(cred)
    } else {
        // This used to delegate error handling to the outer function, but I changed it to do it
        // here because the function had thread-local state associated with it which persisted
        // past the moment it returned — it's part of the function's signature, in some way,
        // that errno contains the error result after the function is called, meaning that
        // leaving usable data in global variables is part of its API, and that's a bad pratice.
        Err(io::Error::last_os_error())
    }
}
pub unsafe fn raw_set_nonblocking(socket: i32, nonblocking: bool) -> io::Result<()> {
    let (old_flags, success) = unsafe {
        // SAFETY: nothing too unsafe about this function. One thing to note is that we're passing
        // it a null pointer, which is, for some reason, required yet ignored for F_GETFL.
        let result = libc::fcntl(socket, F_GETFL, null::<c_void>());
        (result, result != -1)
    };
    if !success {
        return Err(io::Error::last_os_error());
    }
    let new_flags = if nonblocking {
        old_flags | O_NONBLOCK
    } else {
        // Inverting the O_NONBLOCK value sets all the bits in the flag set to 1 except for the
        // nonblocking flag, which clears the flag when ANDed.
        old_flags & !O_NONBLOCK
    };
    let success = unsafe {
        // SAFETY: new_flags is a c_int, as documented in the manpage.
        libc::fcntl(socket, F_SETFL, new_flags)
    } != -1;
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
pub unsafe fn raw_get_nonblocking(socket: i32) -> io::Result<bool> {
    let flags = unsafe {
        // SAFETY: exactly the same as above.
        libc::fcntl(socket, F_GETFL, null::<c_void>())
    };
    if flags != -1 {
        Ok(flags & O_NONBLOCK != 0)
    } else {
        // Again, querying errno was previously left to the outer function but is now done here.
        Err(io::Error::last_os_error())
    }
}
