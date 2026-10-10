#[cfg(unix)]
use color_eyre::eyre::eyre;
use {
    crate::{
        local_socket::{prelude::*, ListenerOptions, Name, Stream},
        tests::util::*,
        BoolExt, SubUsizeExt,
    },
    color_eyre::eyre::{ensure, WrapErr},
    std::{
        io::{BufRead, BufReader, Write},
        str,
        sync::{mpsc::Sender, Arc},
    },
};

fn msg(server: bool, nts: bool) -> Box<str> {
    message(None, server, Some(['\n', '\0'][nts.to_usize()]))
}

fn fork(a: impl FnOnce() -> TestResult, b: impl FnOnce() -> TestResult + Send) -> TestResult {
    std::thread::scope(|scope| {
        let b = scope.spawn(b);
        a()?;
        b.join().unwrap()
    })
}

fn check_peer_creds(s: &Stream) -> TestResult {
    let creds = s.peer_creds().opname("peer_creds")?;

    if cfg!(any(target_os = "linux", target_os = "android", windows)) {
        ensure!(creds.pid().is_some(), "this platform is supposed to provide peer PID");
    }
    #[allow(clippy::cast_sign_loss)]
    if let Some(pid) = creds.pid() {
        ensure_eq!(pid as u32, std::process::id());
    }

    #[cfg(unix)]
    {
        if cfg!(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
        )) {
            ensure!(
                creds.groups().is_some(),
                "this platform is supposed to provide supplementary groups"
            );
        }

        if let Some(creds_groups) = creds.groups() {
            let mut creds_groups = creds_groups.to_owned();
            let mut actual_groups = get_groups().opname("getgroups")?;

            let (trnc, tpad) =
                if creds.groups_truncated() { (" (trunc)", "        ") } else { ("", "") };
            println!(
                "\
groups in peer_creds{trnc} ({:4}): {creds_groups:?}
actual groups       {tpad} ({:4}): {actual_groups:?}",
                creds_groups.len(),
                actual_groups.len(),
            );
            ensure!(!creds.groups_truncated() || creds_groups.len() < actual_groups.len());

            if creds.groups_truncated() {
                for group in &creds_groups {
                    ensure!(actual_groups.contains(group));
                }
            } else {
                creds_groups.sort_unstable();
                actual_groups.sort_unstable();
                // contents already printed
                ensure!(creds_groups == actual_groups);
            }
        }

        // It's easier to debug the below assertion failures after the supplementary groups have
        // been printed.
        let euid = creds.euid().ok_or_else(|| eyre!("missing EUID"))?;
        let egid = creds.egid().ok_or_else(|| eyre!("missing EGID"))?;
        ensure_eq!(euid, unsafe { libc::geteuid() });
        ensure_eq!(egid, unsafe { libc::getegid() });
    }
    Ok(())
}

#[cfg(unix)]
#[allow(clippy::cast_sign_loss)] // negative values are checked
fn get_groups() -> std::io::Result<Vec<libc::gid_t>> {
    let (mut buf, mut buf_size) = (Vec::new(), 0);
    loop {
        let num_groups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
        if num_groups < 0 {
            crate::cold_path();
            return Err(std::io::Error::last_os_error());
        }
        buf_size = std::cmp::max(buf_size, num_groups);
        buf.reserve_exact((buf_size as usize) - buf.capacity());
        let num_groups =
            unsafe { libc::getgroups(buf_size, buf.spare_capacity_mut().as_mut_ptr().cast()) };
        if num_groups < 0 {
            crate::cold_path();
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINVAL) {
                continue;
            }
            return Err(err);
        }
        unsafe { buf.set_len(num_groups as usize) };
        break Ok(buf);
    }
}

#[cfg(unix)]
#[cfg_attr(not(target_os = "freebsd"), allow(dead_code))]
fn os_release() -> Box<[u32]> {
    const RELEASE_OFFSET: usize = offset_of!(libc::utsname, version);
    use std::{ffi::CStr, mem::MaybeUninit};

    let mut utsname = MaybeUninit::uninit();
    if unsafe { libc::uname(utsname.as_mut_ptr()) <= 0 } {
        crate::aborting_panic("uname failed, stack is corrupt");
    }
    // SAFETY: in bounds because the pointer is to a utsname struct
    let release_ptr = unsafe { utsname.as_ptr().cast::<libc::c_char>().byte_add(RELEASE_OFFSET) };
    // SAFETY: the struct is a local variable and the string doesn't escape this function
    let release = unsafe { CStr::from_ptr(release_ptr) }.to_bytes();

    let (mut result, mut current_component) = (Vec::new(), 0);
    for &c in release {
        if c == b'.' {
            result.push(current_component);
            current_component = 0;
        } else if c.is_ascii_digit() {
            current_component = current_component * 10 + u32::from(c - b'0');
        }
    }
    result.into_boxed_slice()
}

pub fn server(
    id: &str,
    handle_client: fn(Stream) -> TestResult,
    name_sender: Sender<Arc<Name<'static>>>,
    num_clients: u32,
    path: bool,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut namegen_local_socket(id, path), |nm| {
        ListenerOptions::new().name(nm.borrow()).create_sync()
    })?;
    let _ = name_sender.send(Arc::new(name));
    listener
        .incoming()
        .take(num_clients.try_into().unwrap())
        .try_for_each(|conn| handle_client(conn.opname("accept")?))
}

pub fn handle_client(conn: Stream) -> TestResult {
    check_peer_creds(&conn)?;
    conn.set_nonblocking(true).opname("set_nonblocking(true)")?;
    conn.set_nonblocking(false).opname("set_nonblocking(false)")?;
    let mut rx = BufReader::new(&conn);
    let mut tx = &conn;
    fork(|| recv(&mut rx, &msg(false, false), 0), || send(&mut tx, &msg(true, false), 0))?;
    fork(|| recv(&mut rx, &msg(false, true), 1), || send(&mut tx, &msg(true, true), 1))?;
    Ok(())
}

pub fn client(name: &Name<'_>) -> TestResult {
    let conn = Stream::connect(name.borrow()).opname("connect")?;
    check_peer_creds(&conn)?;
    let mut rx = BufReader::new(&conn);
    let mut tx = &conn;
    conn.set_nonblocking(true).opname("set_nonblocking(true)")?;
    conn.set_nonblocking(false).opname("set_nonblocking(false)")?;
    fork(|| recv(&mut rx, &msg(true, false), 0), || send(&mut tx, &msg(false, false), 0))?;
    fork(|| send(&mut tx, &msg(false, true), 1), || recv(&mut rx, &msg(true, true), 1))?;
    Ok(())
}

fn recv(conn: &mut dyn BufRead, exp: &str, nr: u8) -> TestResult {
    let term = *exp.as_bytes().last().unwrap();
    let fs = ["first", "second"][nr.to_usize()];

    let mut buffer = Vec::with_capacity(exp.len());
    conn.read_until(term, &mut buffer).wrap_err_with(|| format!("{} receive failed", fs))?;
    ensure_eq!(
        str::from_utf8(&buffer).with_context(|| format!("{} receive wasn't valid UTF-8", fs))?,
        exp,
    );
    Ok(())
}
fn send(conn: &mut dyn Write, msg: &str, nr: u8) -> TestResult {
    let fs = ["first", "second"][nr.to_usize()];
    conn.write_all(msg.as_bytes()).with_context(|| format!("{} socket send failed", fs))
}
