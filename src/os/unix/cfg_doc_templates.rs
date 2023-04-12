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

#[cfg_attr( // uds_peerucred template
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

*/
