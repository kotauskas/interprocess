use {
    super::{downgrade_eof, winprelude::*},
    crate::{mut2ptr, timeout_expiry, AsBuf, CannotUnwind, OrErrno as _, SubUsizeExt as _},
    std::{
        io, ptr,
        time::{Duration, Instant},
    },
    windows_sys::Win32::{
        Foundation::{
            DuplicateHandle, GetLastError, BOOL, DUPLICATE_SAME_ACCESS, ERROR_IO_PENDING,
            ERROR_NOT_FOUND, MAX_PATH, WAIT_IO_COMPLETION,
        },
        Storage::FileSystem::{
            FlushFileBuffers, GetFinalPathNameByHandleW, ReadFile, ReadFileEx, WriteFile,
            WriteFileEx,
        },
        System::{
            Threading::{GetCurrentProcess, SleepEx},
            IO::{CancelIoEx, OVERLAPPED},
        },
    },
};

const OVERLAPPED_INIT: OVERLAPPED = unsafe { std::mem::zeroed() };

#[repr(C)]
struct CompletionResult {
    error_code: u32,
    n_bytes: u32,
}
impl CompletionResult {
    pub fn write_to_overlapped(&mut self, overlapped: &mut OVERLAPPED) {
        overlapped.hEvent = self as *mut _ as isize
    }
    #[allow(clippy::cast_sign_loss)] // not a number
    pub unsafe fn from_overlapped<'s>(overlapped: *mut OVERLAPPED) -> &'s mut Self {
        // FUTURE use Exposed Provenance API
        unsafe { &mut *((*overlapped).hEvent as usize as *mut _) }
    }
    pub fn has_finished(&self) -> bool {
        (self.error_code == 0 && self.n_bytes > 0)
            || (self.error_code != 0 && self.error_code != ERROR_IO_PENDING)
    }
    pub unsafe extern "system" fn routine(code: u32, n_bytes: u32, ov: *mut OVERLAPPED) {
        let resultbuf = unsafe { CompletionResult::from_overlapped(ov) };
        resultbuf.error_code = code;
        resultbuf.n_bytes = n_bytes;
    }
}
impl Default for CompletionResult {
    fn default() -> Self { Self { error_code: ERROR_IO_PENDING, n_bytes: 0 } }
}

pub fn duplicate_handle(handle: BorrowedHandle<'_>) -> io::Result<OwnedHandle> {
    let raw = duplicate_handle_inner(handle, None)?;
    unsafe { Ok(OwnedHandle::from_raw_handle(raw.to_std())) }
}
pub fn duplicate_handle_to_foreign(
    handle: BorrowedHandle<'_>,
    other_process: BorrowedHandle<'_>,
) -> io::Result<HANDLE> {
    duplicate_handle_inner(handle, Some(other_process))
}

fn duplicate_handle_inner(
    handle: BorrowedHandle<'_>,
    other_process: Option<BorrowedHandle<'_>>,
) -> io::Result<HANDLE> {
    let mut new_handle = INVALID_HANDLE_VALUE;
    unsafe {
        let proc = GetCurrentProcess();
        DuplicateHandle(
            proc,
            handle.as_int_handle(),
            other_process.map(|h| h.as_int_handle()).unwrap_or(proc),
            &mut new_handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .true_val_or_errno(new_handle)
}

#[inline]
pub unsafe fn read_ptr(h: BorrowedHandle<'_>, ptr: *mut u8, len: usize) -> io::Result<usize> {
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    let mut bytes_read: u32 = 0;
    unsafe { ReadFile(h.as_int_handle(), ptr, len, mut2ptr(&mut bytes_read), ptr::null_mut()) }
        .true_val_or_errno(bytes_read.to_usize())
}
#[inline]
pub fn read(h: BorrowedHandle<'_>, buf: &mut (impl AsBuf + ?Sized)) -> io::Result<usize> {
    unsafe { read_ptr(h, buf.as_ptr(), buf.len()) }
}

pub fn exsync_op(
    h: BorrowedHandle<'_>,
    mut timeout: Option<Duration>,
    mut f: impl FnMut(&mut OVERLAPPED) -> BOOL,
) -> io::Result<usize> {
    let mut resultbuf = CompletionResult::default();
    let mut ov = OVERLAPPED_INIT;
    resultbuf.write_to_overlapped(&mut ov);
    let end = timeout.map(timeout_expiry).transpose()?;
    'outer: loop {
        let uw_guard = CannotUnwind::begin();
        f(&mut ov).true_val_or_errno(())?;
        // The above early return is okay because we always enter a loop iteration without a
        // pending I/O op, and a zero return value means that it failed to start the operation.
        'wait: loop {
            if wait_apc(timeout) {
                break 'wait;
            }
            if let Some(end) = end {
                let remain = end.saturating_duration_since(Instant::now());
                if remain == Duration::ZERO {
                    cancel_io(h, &ov)
                        .expect("CancelIoEx unexpectedly failed during critical section");
                    uw_guard.end();
                    break 'outer;
                }
                timeout = Some(remain);
            }
        }
        uw_guard.end();
        if resultbuf.has_finished() {
            break 'outer;
        }
    }
    if resultbuf.error_code == 0 {
        Ok(resultbuf.n_bytes.to_usize())
    } else {
        #[allow(clippy::cast_possible_wrap)]
        Err(io::Error::from_raw_os_error(resultbuf.error_code as _))
    }
}

pub unsafe fn read_exsync_ptr(
    h: BorrowedHandle<'_>,
    ptr: *mut u8,
    len: usize,
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let routine = Some(CompletionResult::routine as _);
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    exsync_op(h, timeout, |ov| unsafe { ReadFileEx(h.as_int_handle(), ptr, len, ov, routine) })
}
#[inline]
pub fn read_exsync(
    h: BorrowedHandle<'_>,
    buf: &mut (impl AsBuf + ?Sized),
    timeout: Option<Duration>,
) -> io::Result<usize> {
    unsafe { read_exsync_ptr(h, buf.as_ptr(), buf.len(), timeout) }
}

#[inline]
pub fn write(h: BorrowedHandle<'_>, buf: &[u8]) -> io::Result<usize> {
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    let mut bytes_written: u32 = 0;
    unsafe {
        WriteFile(
            h.as_int_handle(),
            buf.as_ptr().cast(),
            len,
            mut2ptr(&mut bytes_written),
            ptr::null_mut(),
        )
    }
    .true_val_or_errno(bytes_written.to_usize())
}

pub fn write_exsync(
    h: BorrowedHandle<'_>,
    buf: &[u8],
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let routine = Some(CompletionResult::routine as _);
    let ptr = buf.as_ptr();
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    exsync_op(h, timeout, |ov| unsafe { WriteFileEx(h.as_int_handle(), ptr, len, ov, routine) })
}

#[inline]
pub fn flush(h: BorrowedHandle<'_>) -> io::Result<()> {
    downgrade_eof(unsafe { FlushFileBuffers(h.as_int_handle()) }.true_val_or_errno(()))
}

#[allow(clippy::arithmetic_side_effects)]
fn duration_to_timeout(duration: Option<Duration>) -> u32 {
    let Some(duration) = duration else { return u32::MAX };
    let sec_millis = u128::from(duration.as_secs()) * 1000;
    let subsec_millis = duration.subsec_nanos().div_ceil(1_000_000);
    let millis = sec_millis.saturating_add(u128::from(subsec_millis));
    u32::try_from(millis).unwrap_or(u32::MAX - 1)
}
pub fn wait_apc(timeout: Option<Duration>) -> bool {
    unsafe { SleepEx(duration_to_timeout(timeout), 1) == WAIT_IO_COMPLETION }
}

pub fn cancel_io(h: BorrowedHandle<'_>, ov: &OVERLAPPED) -> io::Result<bool> {
    if unsafe { CancelIoEx(h.as_int_handle(), ov) } != 0 {
        return Ok(true);
    }
    let err = unsafe { GetLastError() };
    if err == ERROR_NOT_FOUND {
        Ok(false)
    } else {
        #[allow(clippy::cast_possible_wrap)] // not a number
        Err(io::Error::from_raw_os_error(err as _))
    }
}

pub fn path(h: BorrowedHandle<'_>) -> io::Result<Vec<u16>> {
    let h = h.as_int_handle();
    let mut buf = Vec::with_capacity((MAX_PATH + 1).to_usize());
    loop {
        let bufcap = buf.capacity().try_into().unwrap_or(u32::MAX);
        let rslt = unsafe { GetFinalPathNameByHandleW(h, buf.as_mut_ptr(), bufcap, 0) };
        let false = rslt == 0 else { return Err(io::Error::last_os_error()) };
        if rslt == u32::MAX {
            return Err(io::Error::other("GetFinalPathNameByHandleW returned u32::MAX"));
        }
        #[allow(clippy::arithmetic_side_effects)] // cannot be u32::MAX
        let len_with_nul = rslt.to_usize() + 1;
        if rslt < bufcap {
            unsafe { buf.set_len(len_with_nul) };
            return Ok(buf);
        }
        buf.reserve_exact(len_with_nul);
    }
}
