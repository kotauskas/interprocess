#![cfg(unix)]

#[path = "../util/mod.rs"]
#[macro_use]
mod util;
use util::*;

mod datagram;
mod stream;

#[test]
fn udsocket_stream() {
    use stream::*;
    color_eyre::install().unwrap();
    run_with_namegen(NameGen::new(make_id!(), false));
    if cfg!(target_os = "linux") {
        run_with_namegen(NameGen::new(make_id!(), true));
    }
}

#[test]
fn udsocket_datagram() {
    use datagram::*;
    color_eyre::install().unwrap();
    run_with_namegen(NameGen::new(make_id!(), false));
    if cfg!(target_os = "linux") {
        run_with_namegen(NameGen::new(make_id!(), true));
    }
}
