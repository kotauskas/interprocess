use {
    super::{block_in_new_rt, UdStream, UdStreamListener},
    std::{fs::remove_file, thread},
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        join,
    },
};

static SOCKET_NAME: &str = "/tmp/interprocess_tokio_udstream_test.sock";

#[tokio::test]
async fn main() {
    let _ = remove_file(SOCKET_NAME);
    let listener = UdStreamListener::bind(SOCKET_NAME).expect("listener bind failed");

    let child_thread = thread::spawn(|| block_in_new_rt(client()));

    block_in_new_rt(server(&listener));
    child_thread.join().expect("client thread panicked");
    let _ = remove_file(SOCKET_NAME);
}

async fn server(listener: &UdStreamListener) {
    let mut conn = listener.accept().await.expect("connection accept failed");
    let (mut reader, mut writer) = conn.split();

    let write = async {
        writer
            .write_all(b"Hello from server!")
            .await
            .expect("server write failed");
        writer.shutdown().expect("socket shutdown failed");
    };

    let mut input_string = String::with_capacity(2048);
    let read = async {
        reader
            .read_to_string(&mut input_string)
            .await
            .expect("server read failed");
    };

    join!(write, read);
    drop(conn);

    assert_eq!(input_string, "Hello from client!");
}

async fn client() {
    let mut conn = UdStream::connect(SOCKET_NAME)
        .await
        .expect("failed to connect");
    let (mut reader, mut writer) = conn.split();

    let write = async {
        writer
            .write_all(b"Hello from client!")
            .await
            .expect("client write failed");
        writer.shutdown().expect("socket shutdown failed");
    };

    let mut input_string = String::with_capacity(2048);
    let read = async {
        reader
            .read_to_string(&mut input_string)
            .await
            .expect("client read failed");
    };

    join!(read, write);
    drop(conn);

    assert_eq!(input_string, "Hello from server!");
}
