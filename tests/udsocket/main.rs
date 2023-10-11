#![cfg(unix)]

#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

#[cfg(any(uds_cont_credentials, uds_cmsgcred))]
mod credentials;
mod datagram;
mod stream;

#[test]
fn udsocket_stream() -> TestResult {
    use stream::*;
    install_color_eyre();
    run(NameGen::new(make_id!(), false))?;
    if cfg!(target_os = "linux") {
        run(NameGen::new(make_id!(), true))?;
    }
    Ok(())
}

#[cfg(uds_cont_credentials)]
#[test]
fn udsocket_continuous_credentials() -> TestResult {
    use credentials::*;
    install_color_eyre();
    run(NameGen::new(make_id!(), false), true)?;
    if cfg!(target_os = "linux") {
        run(NameGen::new(make_id!(), true), true)?;
    }
    Ok(())
}

#[cfg(uds_cmsgcred)]
#[test]
fn udsocket_explicitly_sent_credentials() -> TestResult {
    use credentials::*;
    install_color_eyre();
    run(NameGen::new(make_id!(), false), false)?;
    if cfg!(target_os = "linux") {
        run(NameGen::new(make_id!(), true), false)?;
    }
    Ok(())
}

#[test]
fn udsocket_datagram() -> TestResult {
    use datagram::*;
    install_color_eyre();
    run(NameGen::new(make_id!(), false))?;
    if cfg!(target_os = "linux") {
        run(NameGen::new(make_id!(), true))?;
    }
    Ok(())
}
