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

/// This can define the following:
/// - `uds_supported`
/// - Ancillary data support:
///     - `uds_scm_rights` ("passfd")
///     - `uds_scm_credentials` ("passcred")
/// - Credentials structure flavor:
///     - `uds_ucred`
///     - `uds_xucred`
///     - `uds_sockcred` from NetBSD
/// - Socket peer credential accessors:
///     - `uds_peercred`, support for `SO_PEERCRED`
///     - `uds_getpeerucred` as seen on Solaris
///     - `uds_peereid`, exclusive to NetBSD
/// - Address length flavors:
///     - `uds_sockaddr_un_len_108`
///     - `uds_sockaddr_un_len_104`, on the BSD family
///     - `uds_sockaddr_un_len_126`, only on Haiku
/// - `msghdr`'s `msg_iovlen` type:
///     - `uds_msghdr_iovlen_c_int`
///     - `uds_msghdr_iovlen_size_t`, on Linux with GNU, Android, uClibc MIPS64, and uClibc x86-64
/// - `msghdr`'s `msg_controllen` type:
///     - `uds_msghdr_controllen_socklen_t`
///     - `uds_msghdr_controllen_size_t`, on Linux with GNU, Android, uClibc MIPS64, and uClibc x86-64
#[rustfmt::skip]
fn collect_uds_features(target: &TargetTriplet) {
    let (mut uds, mut scm_rights) = (false, true);
    if (target.os("linux") && target.env_any(&["gnu", "musl", "musleabi", "musleabihf"]))
    || target.os_any(&["android", "emscripten", "fuchsia", "redox"]) {
        // "Linux-like" in libc terminology, plus Fuchsia and Redox
        uds = true;
        define("uds_sockaddr_un_len_108");
        if !target.os("emscripten") {
            ldefine(&["uds_ucred", "uds_scm_credentials", "uds_peercred"]);
        }
        if (target.os("linux") && target.env("gnu"))
        || (target.os("linux") && target.env("uclibc") && target.arch_any(&["x86_64", "mips64"]))
        || target.os("android") {
            ldefine(&["uds_msghdr_iovlen_size_t", "uds_msghdr_controllen_size_t"]);
        } else {
            ldefine(&["uds_msghdr_iovlen_c_int", "uds_msghdr_controllen_socklen_t"]);
        }
        if target.os_any(&["linux", "android"]) {
            // Only actual Linux has that... I think? lmao
            define("uds_linux_namespace");
        }
    } else if target.env("newlib") && target.arch("xtensa") {
        uds = true;
        scm_rights = false;
        ldefine(&[
            "sockaddr_un_len_108", "uds_msghdr_iovlen_c_int", "uds_msghdr_controllen_socklen_t",
        ]);
    } else if target.os_any(&["freebsd", "openbsd", "netbsd", "dragonfly", "macos", "ios"]) {
        // The BSD OS family
        uds = true;
        ldefine(&[
            "uds_sockaddr_un_len_104",
            "uds_msghdr_iovlen_c_int",
            "uds_msghdr_controllen_socklen_t",
            "uds_xucred",
        ]);
        if target.os("netbsd") {
            // NetBSD differs from all other BSDs in that it uses its own
            // credential structure, sockcred
            ldefine(&["uds_sockcred", "uds_peereid"]);
        } else if target.os_any(&["freebsd", "dragonfly", "macos", "ios"]) {
            define("uds_xucred");
        }
    } else if target.os_any(&["solaris", "illumos"]) {
        uds = true;
        ldefine(&[
            "uds_sockaddr_un_len_108",
            "uds_getpeerucred",
            "uds_msghdr_iovlen_c_int",
            "uds_msghdr_controllen_socklen_t",
        ]);
    } else if target.os("haiku") {
        uds = true;
        ldefine(&[
            "uds_sockaddr_un_len_126",
            "uds_ucred",
            "uds_peercred",
            "uds_msghdr_iovlen_c_int",
            "uds_msghdr_controllen_socklen_t",
        ]);
    }
    if uds {
        if scm_rights { define("uds_scm_rights") };
        if !target.arch_any(&["x86", "x86_64"]) { define("uds_ancillary_unsound") };
        define("uds_supported");
    }
}

fn define(cfg: &str) {
    ldefine(&[cfg]);
}
fn ldefine(cfgs: &[&str]) {
    let stdout_ = io::stdout();
    let mut stdout = stdout_.lock();
    for i in cfgs {
        stdout.write_all(b"cargo:rustc-cfg=").unwrap();
        stdout.write_all(i.as_ref()).unwrap();
        stdout.write_all(b"\n").unwrap();
    }
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
    fn arch(&self, arch: &str) -> bool { self.arch == arch }
    fn arch_any(&self, arches: &[&str]) -> bool { arches.iter().copied().any(|x| x == self.arch) }
    fn os(&self, os: &str) -> bool { self.os == os }
    fn os_any(&self, oses: &[&str]) -> bool { oses.iter().copied().any(|x| x == self.os) }
    fn env(&self, env: &str) -> bool { self.env.as_deref() == Some(env) }
    fn env_any(&self, envs: &[&str]) -> bool {
        if let Some(env) = self.env.as_deref() {
            envs.iter().copied().any(|x| x == env)
        } else { false }
    }
}
