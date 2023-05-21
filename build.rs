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
/// - `uds_sun_len` on platforms that have the stupid as fuck `sun_len` field (to correct max length calculation)
/// - Ancillary data support:
///     - `uds_scm_rights` ("passfd")
///     - `uds_scm_credentials` ("passcred")
/// - Credential ancillary message structure flavor:
///     - `uds_ucred`
///     - `uds_sockcred`
/// - Socket options for retrieving peer credentials:
///     - `uds_peerucred`
///     - `uds_getpeerucred` as seen on Solaris (the `ucred` in its case is a completely different beast compared to Linux)
///     - `uds_unpcbid`, as seen on NetBSD
///     - `uds_xucred`, as seen on all BSDs except for NetBSD
/// - `msghdr`'s `msg_iovlen` type:
///     - `uds_msghdr_iovlen_c_int`
///     - `uds_msghdr_iovlen_size_t`, on Linux with GNU, AIX, Android, uClibc MIPS64, and uClibc x86-64
/// - `msghdr`'s `msg_controllen` type:
///     - `uds_msghdr_controllen_socklen_t`
///     - `uds_msghdr_controllen_size_t`, on Linux with GNU, AIX, Android, uClibc MIPS64, and uClibc x86-64
/// - `cmsghdr`'s `cmsg_len` type:
///     - `uds_cmsghdr_len_socklen_t`
///     - `uds_cmsghdr_len_size_t`, on Linux with GNU, AIX, Android, uClibc MIPS64, and uClibc x86-64
#[rustfmt::skip]
fn collect_uds_features(target: &TargetTriplet) {
    let (mut uds, mut scm_rights, mut size_t_madness) = (false, true, false);
    if (target.os("linux") && target.env_any(&["gnu", "musl", "musleabi", "musleabihf"]))
    || target.os_any(&["android", "emscripten", "fuchsia", "redox"]) {
        // "Linux-like" in libc terminology, plus Fuchsia and Redox
        uds = true;
        if !target.os("emscripten") {
            ldefine(&["uds_ucred", "uds_scm_credentials", "uds_peerucred"]);
        }
        if (target.os("linux") && target.env("gnu"))
        || (target.os("linux") && target.env("uclibc") && target.arch_any(&["x86_64", "mips64"]))
        || target.os("android") {
            size_t_madness = true;
        }
        if target.os_any(&["linux", "android"]) {
            // Only actual Linux has that... I think? lmao
            define("uds_linux_namespace");
        }
    } else if target.os_any(&["aix", "nto"]) || (target.env("newlib") && target.arch("xtensa")) {
        uds = true;
        if target.os("nto") {
            define("uds_peerucred");
        } else if target.env("newlib") && target.arch("xtensa") {
            scm_rights = false;
        }
    } else if target.os_any(&["freebsd", "openbsd", "netbsd", "dragonfly", "macos", "ios"]) {
        // The BSD OS family
        uds = true;
        ldefine(&[
            "uds_peereid",
            "uds_sun_len",
        ]);
        // FIXME sockcred platforms are really fucked, like actually messed up in the head. They make my brain hurt
        // in one of the worst ways imaginable. They disgust me beyond human belief. Just read the fucking FreeBSD
        // manpage and you'll start wishing for a legally forced removal of Unix from human life. I've never been losing
        // my shit over software this hard in my life. Anyway, if you want to fix this functionality, please make a PR.
        // I'm not touching this again.

        // Commented out to pretend it's not real.
        // if !target.os_any(&["macos", "ios"]) {
        //     define("uds_sockcred");
        // }
        if target.os("netbsd") {
            define("uds_unpcbid");
        } else {
            define("uds_xucred");
        }
    } else if target.os_any(&["solaris", "illumos"]) {
        uds = true;
        define("uds_getpeerucred");
    } else if target.os("haiku") {
        uds = true;
        ldefine(&["uds_ucred", "uds_peerucred"]);
    }

    if uds {
        define("uds_supported");

        if scm_rights { define("uds_scm_rights") };

        if size_t_madness {
            ldefine(&[
                "uds_msghdr_iovlen_size_t", "uds_msghdr_controllen_size_t", "uds_cmsghdr_len_size_t"
            ]);
        } else {
            ldefine(&[
                "uds_msghdr_iovlen_c_int", "uds_msghdr_controllen_socklen_t", "uds_cmsghdr_len_socklen_t"
            ])
        }
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
