#![cfg(unix)]

mod util;
use util::*;

mod stream;

#[test]
fn udsocket_stream() {
    stream::run_with_namegen(NameGen::new(false));
    if cfg!(target_os = "linux") {
        stream::run_with_namegen(NameGen::new(true));
    }
}
