use super::{UdStream, UdStreamListener};
use std::{
    fs::remove_file,
    io::{self, prelude::*},
    net::Shutdown,
    process,
};

#[test]
fn basic_stream() {
    // Unauthorized deletion of a file from a test is fine because the file in
    // question is in tmpfs and therefore is not allowed to hold any long-term
    // significance.
    let _ = remove_file(SOCKET_NAME);
    let listener = UdStreamListener::bind(SOCKET_NAME).unwrap();
    let (success, child) = unsafe {
        let ret = libc::fork();
        (ret != -1, ret)
    };
    if !success {
        panic!("fork failed: {:?}", io::Error::last_os_error());
    }
    if child == 0 {
        basic_stream_client();
        process::exit(0);
    } else {
        basic_stream_server(&listener);
    }
}
static SOCKET_NAME: &str = "/tmp/interprocess_udstream_test_basic.sock";
fn basic_stream_server(listener: &UdStreamListener) {
    let mut conn = listener.accept().unwrap();
    conn.write_all(b"Hello from server!").unwrap();
    conn.shutdown(Shutdown::Write).unwrap();
    let mut input_string = String::new();
    conn.read_to_string(&mut input_string).unwrap();
    println!("Client answered: {}", input_string);
}
fn basic_stream_client() {
    let mut conn = loop {
        match UdStream::connect(SOCKET_NAME) {
            Ok(c) => break c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => panic!("unexpected connect error: {:?}", e),
        }
    };
    let mut string_buffer = String::new();
    conn.read_to_string(&mut string_buffer).unwrap();
    println!("Server answered: {}", string_buffer);
    conn.write_all(b"Hello from client!").unwrap();
}
