/*
// You can't generate those with macros just yet, so copypasting is the way for now.

#[cfg_attr( // uds_ucred template
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )))
)]

#[cfg_attr( // uds_cmsgcred template
    feature = "doc_cfg",
    doc(cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly"
    )))
)]

#[cfg_attr( // uds_sockcred template
    feature = "doc_cfg",
    doc(cfg(target_os = "freebsd"))
)]

#[cfg_attr( // uds_linux_namespace template
    feature = "doc_cfg",
    doc(cfg(any(target_os = "linux", target_os = "android")))
)]

*/
