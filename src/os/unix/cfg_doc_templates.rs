/*
// You can't generate those with macros just yet, so copypasting is the way for now.

#[cfg_attr( // uds_ucred template
    feature = "doc_cfg",
    doc(cfg(any(
        all(
            target_os = "linux",
            any(
                target_env = "gnu",
                target_env = "musl",
                target_env = "musleabi",
                target_env = "musleabihf"
            )
        ),
        target_os = "emscripten",
        target_os = "redox"
    )))
)]

#[cfg_attr( // uds_linux_namespace template
    feature = "doc_cfg",
    doc(cfg(any(target_os = "linux", target_os = "android")))
)]

#[cfg_attr( // uds_peercred template
    feature = "doc_cfg",
    doc(cfg(any(
        all(
            target_os = "linux",
            any(
                target_env = "gnu",
                target_env = "musl",
                target_env = "musleabi",
                target_env = "musleabihf"
            )
        ),
        target_os = "emscripten",
        target_os = "redox",
        target_os = "haiku"
    )))
)]

#[cfg_attr( // any(se_sigpoll, se_sigpoll_is_sigio) template
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "emscripten",
        target_os = "redox",
        target_os = "haiku",
        target_os = "solaris",
        target_os = "illumos"
    )))
)]

#[cfg_attr( // se_full_posix_1990/se_base_posix_2001 template
    feature = "doc_cfg",
    doc(cfg(not(target_os = "hermit"))),
)]

*/
