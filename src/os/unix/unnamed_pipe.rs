use super::FdOps;
use crate::{
    unnamed_pipe::{UnnamedPipeReader as PubReader, UnnamedPipeWriter as PubWriter},
    Sealed,
};
use libc::c_int;
use std::{
    fmt::{self, Debug, Formatter},
    io,
    os::{
        fd::OwnedFd,
        unix::io::{AsRawFd, FromRawFd},
    },
};

pub(crate) fn pipe() -> io::Result<(PubWriter, PubReader)> {
    let (success, fds) = unsafe {
        let mut fds: [c_int; 2] = [0; 2];
        let result = libc::pipe(fds.as_mut_ptr());
        (result == 0, fds)
    };
    if success {
        let (w, r) = unsafe {
            // SAFETY: we just created both of those file descriptors, which means that neither of
            // them can be in use elsewhere.
            let w = OwnedFd::from_raw_fd(fds[1]);
            let r = OwnedFd::from_raw_fd(fds[0]);
            (w, r)
        };
        let w = PubWriter(UnnamedPipeWriter(FdOps(w)));
        let r = PubReader(UnnamedPipeReader(FdOps(r)));
        Ok((w, r))
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) struct UnnamedPipeReader(FdOps);
impl Sealed for UnnamedPipeReader {}
impl Debug for UnnamedPipeReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeReader")
            .field("fd", &self.0 .0.as_raw_fd())
            .finish()
    }
}
multimacro! {
    UnnamedPipeReader,
    forward_rbv(FdOps, &),
    forward_sync_ref_read,
    forward_try_clone,
    forward_handle,
    derive_sync_mut_read,
}

pub(crate) struct UnnamedPipeWriter(FdOps);
impl Sealed for UnnamedPipeWriter {}
impl Debug for UnnamedPipeWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnnamedPipeWriter")
            .field("fd", &self.0 .0.as_raw_fd())
            .finish()
    }
}

multimacro! {
    UnnamedPipeWriter,
    forward_rbv(FdOps, &),
    forward_sync_ref_write,
    forward_try_clone,
    forward_handle,
    derive_sync_mut_write,
}
