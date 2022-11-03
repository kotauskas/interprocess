#![cfg(unix)]

#[path = "../util/mod.rs"]
mod util;
use util::*;

mod datagram;
mod stream;

#[test]
fn udsocket_stream() {
    use stream::*;
    run_with_namegen(NameGen::new(false));
    if cfg!(target_os = "linux") {
        run_with_namegen(NameGen::new(true));
    }
}

#[test]
fn udsocket_datagram() {
    use datagram::*;
    run_with_namegen(NameGen::new(false));
    if cfg!(target_os = "linux") {
        run_with_namegen(NameGen::new(true));
    }
}
