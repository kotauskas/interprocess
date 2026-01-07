use {
    super::{libc_wrappers::*, *},
    libc::{
        dev_t, gid_t, ino_t, mode_t, off_t, pid_t, sockaddr_storage, sockaddr_un, socklen_t,
        time_t, uid_t,
    },
    std::{
        borrow::Cow,
        ffi::CString,
        fmt::{self, Display, Formatter},
        io,
        mem::ManuallyDrop,
        os::unix::{
            ffi::OsStrExt as _,
            net::{UnixListener, UnixStream},
            prelude::*,
        },
        path::Path,
    },
};

pub(super) fn main() {
    print_bitwidths(&bitwidths!(
        socklen_t, dev_t, ino_t, mode_t, pid_t, uid_t, gid_t, off_t, time_t
    ));
    print_sizes(&sizes!(sockaddr_storage, sockaddr_un));
    println!();

    if let Some(tmpdir) = select_tmpdir() {
        requires_tmpdir(tmpdir);
    }
}

fn requires_tmpdir(tmpdir: Cow<'static, Path>) {
    fn listen(
        path: &Path,
        mask: Option<mode_t>,
        fchmod_mode: Option<mode_t>,
        first: bool,
    ) -> io::Result<UnixListener> {
        let _guard = mask.map(umask);

        let pathdisp = path.display();
        // FIXME(3.0.0) use format_args!
        let err = format!("Failed to bind to {pathdisp}");

        let bind = || {
            let mut report_err = true;
            if let Some(mode) = fchmod_mode {
                bind_with_hook(path, |fd| {
                    fchmod(fd, mode)
                        .report_error("Failed to fchmod listener before bind")
                        .set_if_error(&mut report_err, false)
                })
            } else {
                UnixListener::bind(path)
            }
            .report_error_if(report_err, &err)
        };
        let print_success = || {
            print!("Successfully bound listener");
            if let Some(mask) = mask {
                print!(" with umask {mask:0>3o}");
            }
            if let Some(mode) = fchmod_mode {
                print!("with pre-bind fchmod to {mode:0>4o}");
            }
            print!(" to {pathdisp}");
        };

        if let Ok(listener) = bind() {
            if !first {
                print!("[Caution] ");
            }
            print_success();
            if !first {
                print!(" - previous socket file was overwritten");
            }
            println!();
            return Ok(listener);
        }
        std::fs::remove_file(path).report_error("Failed to remove stale socket")?;
        let rslt = bind().report_error(&err);
        if rslt.is_ok() {
            print_success();
            println!(" after unlinking previous socket");
        }
        rslt
    }
    let listen_path = ManuallyDrop::new(tmpdir.join("interprocess-inspect-platform.sock"));
    let mut fchmod_succeeded = false;
    if let Ok(listener) = listen(&listen_path, Some(0o000), None, true) {
        inspect_listener(&listen_path, listener, 0o777, Some(&mut fchmod_succeeded));
    }
    if let Ok(listener) = listen(&listen_path, Some(0o777), None, false) {
        inspect_listener(&listen_path, listener, 0o000, None);
    }
    if let Ok(listener) = listen(&listen_path, None, Some(0o000), false) {
        inspect_listener(&listen_path, listener, 0o000, None);
    }
    let _ = std::fs::remove_file(&*listen_path).report_error("Failed to remove socket");
}

fn inspect_listener(
    listen_path: &Path,
    listener: UnixListener,
    orig_mode: mode_t,
    fchmod_succeeded: Option<&mut bool>,
) {
    let mut expected_listener_mode = orig_mode;

    let path_cstring = CString::new(listen_path.as_os_str().as_bytes().to_owned()).unwrap();
    let chk_stat = |fchmodded: bool| {
        let fchmodded = if fchmodded { " (fchmodded)" } else { "" };
        if let Ok(stat) = fstat(listener.as_fd()).report_error("Failed to fstat listener") {
            let stat = StatDisplay(stat);
            println!("Listener fstat{fchmodded}: {stat}");
        }
        if let Ok(stat) = stat(&path_cstring).report_error("Failed to stat listener") {
            let stat = StatDisplay(stat);
            println!("Listener  stat{fchmodded}: {stat}");
        }
    };
    chk_stat(false);

    if let Some(fchmod_succeeded) = fchmod_succeeded {
        if fchmod(listener.as_fd(), 0).report_error("Failed to fchmod listener").is_ok() {
            *fchmod_succeeded = true;
            expected_listener_mode = 0o000;
            println!("Changing listener mode to 000 returned success");
        }
        chk_stat(true);

        if UnixStream::connect(listen_path)
            .report_error("Failed to connect to listener after fchmod")
            .is_ok()
        {
            #[rustfmt::skip] println!("\
[Caution] Successfully connected to listener after fchmod to 000 - it is \
likely that the mode set by fchmod only applies when creating the socket \
file during bind()"
            );
        }

        if fchmod(listener.as_fd(), 0o666).report_error("Failed to fchmod listener").is_ok() {
            expected_listener_mode = 0o666;
            println!("Changing listener mode to 666 returned success");
        }
    }

    if UnixStream::connect(listen_path).report_error("Failed to connect to listener").is_ok() {
        let is_prohibited = expected_listener_mode & 0b110_000_000 == 0;
        if is_prohibited {
            print!("[Caution] ");
        }
        print!("Successfully connected to listener");
        if is_prohibited {
            print!(" - privilege checking on socket files presumably inoperative");
        }
        println!();
    }
    println!();
}

fn select_tmpdir() -> Option<Cow<'static, Path>> {
    fn try_var(var: Option<&str>) -> Option<(Option<&str>, Cow<'static, Path>)> {
        let val = if let Some(var) = var {
            let Some(val) = std::env::var_os(var) else {
                println!("{var} is unset");
                return None;
            };
            Cow::Owned(val.into())
        } else {
            Cow::Borrowed(Path::new("/tmp"))
        };
        let print_val = || {
            if let Some(var) = var {
                print!("{var} = ");
            }
            print!("{}", val.display());
        };
        let is_dir = match std::fs::metadata("/tmp") {
            Ok(md) => md.file_type().is_dir(),
            Err(e) => {
                print!("Could not check ");
                print_val();
                println!(" for whether it is a directory or not: {e}");
                return None;
            }
        };
        if !is_dir {
            print_val();
            println!(" is not a directory");
            return None;
        }
        Some((var, val))
    }
    let rslt = try_var(Some("TMPDIR"))
        .or_else(|| try_var(Some("TEMPDIR")))
        .or_else(|| try_var(Some("TMP")))
        .or_else(|| try_var(Some("TEMP")))
        .or_else(|| try_var(None))
        .map(|(var, val)| {
            print!("Using temporary directory ");
            if let Some(var) = var {
                print!("{var} = ");
            }
            println!("{}", val.display());
            val
        });
    println!();
    rslt
}

struct StatDisplay(libc::stat);
impl Display for StatDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let &StatDisplay(libc::stat { st_dev, st_ino, st_mode, st_uid, st_gid, .. }) = self;
        let (majdev, mindev) = (libc::major(st_dev), libc::minor(st_dev));
        write!(
            f,
            "[dev {majdev:>3}:{mindev:0>3}] [ino {st_ino:>8}] [mode {st_mode:0>6o}] [uid {st_uid:>8}] [gid {st_gid:>8}]"
        )
    }
}

trait ResultExt: Sized {
    type Ok;
    type Err: Display;
    fn get_err(&self) -> Option<&Self::Err>;
    fn unwrap_or_else(self, f: impl FnOnce(Self::Err) -> Self::Ok) -> Self::Ok;

    fn report_error(self, msg: &str) -> Self {
        report_error_str(self.get_err(), msg);
        self
    }
    fn report_error_if(self, toggle: bool, msg: &str) -> Self {
        if toggle {
            report_error_str(self.get_err(), msg);
        }
        self
    }
    fn report_error_args(self, msg: fmt::Arguments<'_>) -> Self {
        report_error(self.get_err(), msg);
        self
    }
    fn report_error_args_if(self, toggle: bool, msg: fmt::Arguments<'_>) -> Self {
        if toggle {
            report_error(self.get_err(), msg);
        }
        self
    }
    fn set_if_error<T>(self, out: &mut T, val: T) -> Self {
        if self.get_err().is_some() {
            *out = val;
        }
        self
    }
    fn unwrap_or_exit(self, msg: &str) -> Self::Ok {
        report_error_str(self.get_err(), msg);
        self.unwrap_or_else(forget_error_and_die)
    }
    fn unwrap_or_exit_args(self, msg: fmt::Arguments<'_>) -> Self::Ok {
        report_error(self.get_err(), msg);
        self.unwrap_or_else(forget_error_and_die)
    }
}
#[inline(never)]
fn report_error<E: Display>(e: Option<&E>, msg: fmt::Arguments<'_>) {
    if let Some(e) = e {
        println!("{msg}: {e}");
    }
}
#[inline(never)]
fn report_error_str<E: Display>(e: Option<&E>, msg: &str) {
    if let Some(e) = e {
        println!("{msg}: {e}");
    }
}
fn forget_error_and_die<T, E>(e: E) -> T {
    std::mem::forget(e);
    std::process::exit(1);
}
impl<T, E: Display> ResultExt for Result<T, E> {
    type Ok = T;
    type Err = E;
    fn get_err(&self) -> Option<&E> { self.as_ref().err() }
    fn unwrap_or_else(self, f: impl FnOnce(E) -> T) -> T { self.unwrap_or_else(f) }
}
