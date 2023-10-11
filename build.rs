use std::{
    env::{var as env_var, var_os as env_var_os},
    io::{self, Write},
};

fn main() {
    if is_unix() {
        let target = TargetTriplet::fetch();
        collect_uds_features(&target);
    }
}

fn is_unix() -> bool {
    env_var_os("CARGO_CFG_UNIX").is_some()
}

fn define(cfg: &str) {
    ldefine(&[cfg]);
}
fn ldefine(cfgs: &[&str]) {
    let stdout_ = io::stdout();
    let mut stdout = stdout_.lock();
    for &i in cfgs {
        writeln!(stdout, "cargo:rustc-cfg={i}").unwrap();
    }
}
macro_rules! uds_cdefine {
    ($($nm:ident),+ $(,)?) => {$(
        if $nm {
            define(concat!("uds_", stringify!($nm)))
        }
    )+};
}

/// This can define the following:
/// - `uds_sun_len` on platforms that have the stupid as fuck `sun_len` field (to correct max length calculation)
/// - `uds_sock_cloexec` on platforms with SOCK_CLOEXEC
/// - `uds_sock_nonblock` on platforms with SOCK_NONBLOCK
/// - Credential ancillary message structure flavor:
///     - `uds_ucred` from Linux
///     - `uds_cmsgcred` from FreeBSD
///     - `uds_sockcred2`, also from FreeBSD
///     - `uds_sockcred` from NetBSD ( TODO )
/// - Socket options for retrieving peer credentials:
///     - `uds_getpeerucred` as seen on Solaris (the `ucred` in its case is a completely different beast compared to
///       Linux)
///     - `uds_unpcbid`, as seen on NetBSD
///     - `uds_sockpeercred`, as seen on OpenBSD
///     - `uds_xucred`, as seen on DragonFly BSD, FreeBSD and Apple platforms
/// - `msghdr`'s `msg_iovlen` type:
///     - `uds_msghdr_iovlen_c_int`
///     - `uds_msghdr_iovlen_size_t`
/// - `msghdr`'s `msg_controllen` type:
///     - `uds_msghdr_controllen_socklen_t`
///     - `uds_msghdr_controllen_size_t`
/// - `cmsghdr`'s `cmsg_len` type:
///     - `uds_cmsghdr_len_socklen_t`
///     - `uds_cmsghdr_len_size_t`
#[rustfmt::skip]
fn collect_uds_features(target: &TargetTriplet) {
    let [
        mut size_t_madness,
        mut ucred,
        mut cmsgcred,
        mut sockcred,
        mut sockcred2,
        mut xucred,
        mut sock_cloexec,
        mut sock_nonblock] = [false; 8];
    if target.os_any(&["linux", "android", "fuchsia", "redox"]) {
        // "Linux-like" in libc terminology, plus Fuchsia and Redox
        [ucred, sock_cloexec, sock_nonblock] = [true; 3];
        if (target.os("linux") && target.env("gnu"))
        || (target.os("linux") && target.env("uclibc") && target.arch_any(&["x86_64", "mips64"]))
        || target.os("android") {
            size_t_madness = true;
        }
        if target.os_any(&["linux", "android"]) {
            // Only actual Linux has that... I think? lmao
            define("uds_linux_namespace");
        }
    } else if target.os_any(&["freebsd", "openbsd", "netbsd", "dragonfly", "macos", "ios", "tvos", "watchos"]) {
        // The BSD OS family
        ldefine(&[
            "uds_peereid",
            "uds_sun_len",
        ]);

        if target.os_any(&["freebsd", "dragonfly"]) {
            cmsgcred = true;
            [sock_cloexec, sock_nonblock] = [true; 2];
            if target.os("freebsd") {
                sockcred2 = true;
            }
        }
        if target.os("netbsd") {
            // TODO does it have sock_cloexec and sock_nonblock?
            sockcred = true;
            // TODO
            define("uds_unpcbid");
        } else if target.os("openbsd") {
            // TODO
            define("uds_sockpeercred");
        } else {
            // TODO
            xucred = true;
        }
    } else if target.os_any(&["solaris", "illumos"]) {
        // TODO
        define("uds_getpeerucred");
    }

    if size_t_madness {
        ldefine(&[
            "uds_msghdr_iovlen_size_t", "uds_msghdr_controllen_size_t", "uds_cmsghdr_len_size_t"
        ]);
    } else {
        ldefine(&[
            "uds_msghdr_iovlen_c_int", "uds_msghdr_controllen_socklen_t", "uds_cmsghdr_len_socklen_t"
        ])
    }
    if ucred || cmsgcred || sockcred || sockcred2 || xucred {
        define("uds_credentials");
        let mut contcred = false; // TODO is NetBSD sockcred a contcred?
        contcred |= ucred | sockcred2;
        if ucred || cmsgcred || sockcred || sockcred2 {
            define("uds_ancillary_credentials");
        }
        uds_cdefine!(ucred, cmsgcred, sockcred, sockcred2, contcred, xucred);
    }
    uds_cdefine!(sock_cloexec, sock_nonblock);
}

struct TargetTriplet {
    arch: String,
    os: String,
    env: Option<String>,
}
#[rustfmt::skip]
impl TargetTriplet {
    fn fetch() -> Self {
        Self {
            arch: env_var("CARGO_CFG_TARGET_ARCH").unwrap(),
            os: env_var("CARGO_CFG_TARGET_OS").unwrap(),
            env: env_var("CARGO_CFG_TARGET_ENV").ok(),
        }
    }
    // fn arch(&self, arch: &str) -> bool { self.arch == arch }
    fn arch_any(&self, arches: &[&str]) -> bool { arches.iter().copied().any(|x| x == self.arch) }
    fn os(&self, os: &str) -> bool { self.os == os }
    fn os_any(&self, oses: &[&str]) -> bool { oses.iter().copied().any(|x| x == self.os) }
    fn env(&self, env: &str) -> bool { self.env.as_deref() == Some(env) }
    // fn env_any(&self, envs: &[&str]) -> bool {
    //     if let Some(env) = self.env.as_deref() {
    //         envs.iter().copied().any(|x| x == env)
    //     } else { false }
    // }
}
