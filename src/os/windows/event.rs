use {
    crate::{os::windows::winprelude::*, BoolExt, HandleOrErrno, OrErrno},
    std::{io, ptr},
    windows_sys::Win32::{
        Foundation::{WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{CreateEventW, ResetEvent, SetEvent, WaitForSingleObject},
    },
};

/// Unnamed event object.
///
/// This is currently an implementation detail, but it might be worth extending it with support
/// for named event objects and exposing it in the public API.
#[derive(Debug)]
pub struct Event(AdvOwnedHandle);
impl Event {
    /// Creates an unnamed event object with the specified presence or absence of auto-reset
    /// behavior and the specified initial state.
    #[inline]
    pub fn new(auto_reset: bool, begin_signaled: bool) -> io::Result<Self> {
        unsafe {
            CreateEventW(
                ptr::null(),
                (!auto_reset).to_i32(),
                begin_signaled.to_i32(),
                ptr::null(),
            )
        }
        .handle_or_errno()
        // SAFETY: we just created this handle
        .map(|h| Self(unsafe { AdvOwnedHandle::from_raw_handle(h) }))
    }
    pub fn wait(&self) -> io::Result<()> {
        match unsafe { WaitForSingleObject(self.as_raw_handle(), u32::MAX) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(forever_wait_timed_out()),
            WAIT_ABANDONED => Err(no_wait_abandoned()),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            _ => Err(unknown_wfso_retval()),
        }
    }
    /// Puts the event object into the signaled state.
    #[inline]
    pub fn signal(&self) -> io::Result<()> {
        unsafe { SetEvent(self.as_raw_handle()) }.true_val_or_errno(())
    }
    /// Resets the event object into the non-signaled state.
    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        unsafe { ResetEvent(self.as_raw_handle()) }.true_val_or_errno(())
    }
}

multimacro! {
    Event,
    derive_raw,
    forward_handle,
    forward_try_clone,
}

#[cold]
fn no_wait_abandoned() -> io::Error {
    io::Error::other("WaitForSingleObject returned WAIT_ABANDONED on an event object")
}
#[cold]
fn forever_wait_timed_out() -> io::Error {
    io::Error::other("WaitForSingleObject returned WAIT_TIMEOUT with an infinite timeout")
}
#[cold]
fn unknown_wfso_retval() -> io::Error {
    io::Error::other("WaitForSingleObject returned an unknown value")
}
