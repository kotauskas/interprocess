//! Local sockets implemented using Unix domain sockets.

mod listener;
mod stream;

pub use {listener::*, stream::*};

/// Async Local sockets for Tokio implemented using Unix domain sockets.
#[cfg(feature = "tokio")]
pub mod tokio {
    mod listener;
    mod stream;
    pub use {listener::*, stream::*};
}

use {
    crate::{
        assume_nonzero_slice, check_nonzero_slice,
        local_socket::{Name, NameInner},
        os::unix::{
            ud_addr::{name_too_long, TerminatedUdAddr, UdAddr, SUN_LEN},
            unixprelude::*,
        },
    },
    std::{ffi::OsStr, io, mem::MaybeUninit, num::NonZeroU8, path::Path},
};

#[derive(Clone, Debug, Default)]
struct ReclaimGuard(Option<Name<'static>>);
impl ReclaimGuard {
    fn new(name: Name<'static>) -> Self { Self(if name.is_path() { Some(name) } else { None }) }
    #[cfg_attr(not(feature = "tokio"), allow(dead_code))]
    fn take(&mut self) -> Self { Self(self.0.take()) }
    fn forget(&mut self) { self.0 = None; }
}
impl Drop for ReclaimGuard {
    fn drop(&mut self) {
        if let Self(Some(Name(NameInner::UdSocketPath(path)))) = self {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn check_no_nul(s: &[u8]) -> io::Result<&[NonZeroU8]> {
    let msg = "interior nul bytes are not allowed inside Unix domain socket names";
    check_nonzero_slice(s).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, msg))
}

fn dispatch_name<T>(
    name: Name<'_>,
    is_listener: bool,
    mut create: impl FnMut(TerminatedUdAddr<'_>) -> io::Result<T>,
) -> io::Result<T> {
    let mut addr = UdAddr::new();
    match name.0 {
        NameInner::UdSocketPath(path) => {
            addr.init(check_no_nul(path.as_bytes())?)?;
            create(addr.write_terminator())
        }

        NameInner::UdSocketPseudoNs(name) => {
            let name = name.as_bytes();
            write_run_user(&mut addr, name)?;
            match with_missing_dir_creation(is_listener, addr.write_terminator(), &mut create) {
                Err(e) if fail_is_benign(&e) => {
                    write_prefixed(&mut addr, tmpdir(), name)?;
                    with_missing_dir_creation(is_listener, addr.write_terminator(), &mut create)
                }
                otherwise => otherwise,
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        NameInner::UdSocketNs(name) => {
            addr.init_namespaced(check_no_nul(&name)?)?;
            create(addr.write_terminator())
        }
    }
}

fn with_missing_dir_creation<T>(
    create: bool,
    addr: TerminatedUdAddr<'_>,
    mut f: impl FnMut(TerminatedUdAddr<'_>) -> io::Result<T>,
) -> io::Result<T> {
    match f(addr) {
        Err(e) if create && fail_is_benign(&e) && create_missing_dirs(addr) => f(addr),
        otherwise => otherwise,
    }
}

fn create_missing_dirs(addr: TerminatedUdAddr<'_>) -> bool {
    let path = Path::new(OsStr::from_bytes(addr.inner().path()));
    if let Some(p) = path.parent() {
        if !p.as_os_str().is_empty() {
            if let Ok(()) = std::fs::create_dir_all(p) {
                return true;
            }
        }
    }
    false
}

#[allow(clippy::as_conversions)]
const MAX_RUN_USER: usize = "/run/user//".len() + uid_t::MAX.ilog10() as usize + 1;
const RUN_USER_BUF: usize = MAX_RUN_USER + 1;
const NMCAP: usize = SUN_LEN - MAX_RUN_USER;

#[allow(clippy::as_conversions, clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn write_run_user(addr: &mut UdAddr, name: &[u8]) -> io::Result<()> {
    // Comparing without regard for the length of the /run/user path
    // improves robustness of programs by preventing reliance on the
    // UID being small
    if name.len() > NMCAP {
        return Err(name_too_long());
    }
    addr.reset_len();
    // SAFETY: proof by look at it
    let start = unsafe { assume_nonzero_slice(b"/run/user/") };
    // SAFETY: bounds check just up above
    unsafe { addr.push_slice(start) };
    let uid_len = {
        // SAFETY: we do not write MaybeUninit::uninit
        let buf = unsafe { addr.path_buf_mut() };
        let mut idx = start.len();
        // SAFETY: always safe
        let mut uid = unsafe { libc::getuid() };
        loop {
            buf[idx] = MaybeUninit::new((uid % 10) as u8 + b'0');
            uid /= 10;
            idx += 1;
            if uid == 0 {
                break;
            }
        }
        buf[idx] = MaybeUninit::new(b'/');
        buf[start.len()..idx].reverse();
        idx + 1 - start.len()
    };
    // SAFETY: we just wrote this many bytes to the buffer
    unsafe { addr.incr_len(uid_len) };
    let esc_start = addr.len();
    // SAFETY: bounds check at the beginning
    unsafe { addr.push_slice_with_nuls(name) };
    escape_nuls(&mut addr.path_mut()[esc_start..]);
    Ok(())
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn write_prefixed(addr: &mut UdAddr, pfx: &[NonZeroU8], name: &[u8]) -> io::Result<()> {
    if pfx.len() + name.len() > SUN_LEN {
        return Err(name_too_long());
    }
    let name = check_no_nul(name)?;
    addr.reset_len();
    // SAFETY: bounds check up above
    unsafe { addr.push_slice(pfx) };
    unsafe { addr.push_slice(name) };
    escape_nuls(&mut addr.path_mut()[pfx.len()..]);
    Ok(())
}

fn escape_nuls(b: &mut [u8]) {
    // Previous versions of Interprocess escape nuls with underscores,
    // so do what we've already been doing instead of erroring out
    b.iter_mut().filter(|c| **c == 0).for_each(|c| *c = b'_');
}

fn fail_is_benign(e: &io::Error) -> bool {
    use io::ErrorKind::*;
    matches!(e.kind(), NotFound | Unsupported) || e.raw_os_error() == Some(libc::ENOTDIR)
}

// Don't check this in right away.
fn tmpdir<'p>() -> &'p [NonZeroU8] {
    if cfg!(target_os = "android") {
        // SAFETY: there is a nul terminator
        let mut ptr = unsafe { libc::getenv(b"TMPDIR\0".as_ptr().cast()) };
        if ptr.is_null() {
            // SAFETY: there is a nul terminator
            ptr = unsafe { libc::getenv(b"TEMPDIR\0".as_ptr().cast()) };
        }
        if ptr.is_null() {
            // SAFETY: proof by look at it
            return unsafe { assume_nonzero_slice(b"/data/local/tmp/") };
        }
        // SAFETY: we got it from getenv and it's not null
        let len = unsafe { libc::strlen(ptr) };
        // SAFETY: there are no interior nuls as per strlen, and the pointer
        // is from getenv (meaning it would only be invalidated by setenv,
        // which would be a race)
        unsafe { std::slice::from_raw_parts(ptr.cast(), len) }
    } else {
        // SAFETY: proof by look at it
        unsafe { assume_nonzero_slice(b"/tmp/") }
    }
}
