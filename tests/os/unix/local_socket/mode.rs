use {
    crate::{
        local_socket::{traits::Stream as _, Listener, ListenerOptions, Name, NameInner, Stream},
        os::unix::local_socket::ListenerOptionsExt,
        tests::util::*,
        OrErrno,
    },
    color_eyre::eyre::{bail, Context},
    libc::mode_t,
    std::{io, mem::zeroed, os::unix::prelude::*, path::Path},
};

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn get_file_mode(path: &Path) -> TestResult<mode_t> {
    Ok(std::fs::symlink_metadata(path)?.mode() as mode_t & 0o777)
}
fn get_fd_mode(fd: BorrowedFd<'_>) -> TestResult<mode_t> {
    let mut stat = unsafe { zeroed::<libc::stat>() };
    unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) != -1 }
        .true_val_or_errno(())
        .opname("stat")?;
    Ok(stat.st_mode & 0o777)
}

fn test_for_mode(path: bool, mode: mode_t) -> TestResult<bool> {
    let (name, listener) =
        listen_and_pick_name(&mut namegen_local_socket(make_id!(), path), |nm| {
            let rslt =
                ListenerOptions::new().name(nm.borrow()).mode(mode).create_sync().map(Some);
            if rslt.as_ref().is_err_and(|e| e.kind() == io::ErrorKind::Unsupported)
                && cfg!(not(any(
                    target_os = "android",
                    target_os = "freebsd",
                    target_os = "linux",
                )))
            {
                return Ok(None);
            }
            rslt
        })?;

    // If listener is None, we're on a platform on which we expect this to not be supported
    let Some(listener) = listener else { return Ok(false) };

    let got_mode = if let Name(NameInner::UdSocketPath(path)) = &name {
        get_file_mode(Path::new(path))
    } else {
        // Abstract namespace, cannot stat a file
        let Listener::UdSocket(listener) = &listener;
        get_fd_mode(listener.as_fd())
    }
    .opname("get mode")?;
    eprintln!("got mode {got_mode:3o}");
    ensure_eq!(got_mode & !mode, 0);

    let should_succeed = mode & 0o200 != 0; // our docs claim that the write bit is checked
    let rslt = Stream::connect(name.borrow());
    if should_succeed {
        rslt.opname("client connect")?;
    } else if rslt.is_ok() {
        bail!("connection succeeded despite file mode {mode:3o} prohibiting it");
    }

    Ok(true)
}

fn test_main(path: bool) -> TestResult {
    const MODES: &[mode_t] = &[
        0o777, 0o770, 0o700, 0o766, 0o666, 0o660, 0o600, 0o500, 0o400, 0o300, 0o200, 0o100, 0o000,
    ];
    for &mode in MODES {
        let is_supported = !test_for_mode(path, mode)
            .with_context(|| format!("test for mode {mode:3o} failed"))?;
        let true = is_supported else { break };
    }
    Ok(())
}

#[test]
fn file_main() -> TestResult { test_wrapper(|| test_main(true)) }

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn namespaced_main() -> TestResult { test_wrapper(|| test_main(false)) }
