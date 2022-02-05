use {
    super::{UdStream, UdStreamListener},
    std::{fs::remove_file, io::prelude::*, net::Shutdown, thread},
};

static SOCKET_NAME: &str = "/tmp/interprocess_udstream_test.sock";

#[test]
fn main() {
    let _ = remove_file(SOCKET_NAME);
    let listener = UdStreamListener::bind(SOCKET_NAME).expect("listener bind failed");

    let client_thread = thread::spawn(client);

    server(&listener);
    drop(listener);

    client_thread.join().expect("client thread panicked");

    let _ = remove_file(SOCKET_NAME);
}

fn server(listener: &UdStreamListener) {
    let mut conn = listener.accept().expect("connection accept failed");

    conn.write_all(b"Hello from server!")
        .expect("server write failed");
    conn.shutdown(Shutdown::Write)
        .expect("socket shutdown failed");

    let mut input_string = String::with_capacity(2048);
    conn.read_to_string(&mut input_string)
        .expect("server read failed");

    assert_eq!(input_string, "Hello from client!");
}

fn client() {
    let mut conn = UdStream::connect(SOCKET_NAME).expect("connect failed");

    let mut string_buffer = String::with_capacity(2048);
    conn.read_to_string(&mut string_buffer)
        .expect("client read failed");

    conn.write_all(b"Hello from client!")
        .expect("client write failed");

    assert_eq!(string_buffer, "Hello from server!");
}
