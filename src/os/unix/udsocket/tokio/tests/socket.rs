use {
    super::{block_in_new_rt, UdSocket},
    std::{fs::remove_file, thread},
    tokio::{io::ReadBuf, join},
};

static SOCKET_A: &str = "/tmp/interprocess_tokio_udsocket_test_a.sock";
static SOCKET_B: &str = "/tmp/interprocess_tokio_udsocket_test_b.sock";

#[test]
fn main() {
    let _ = remove_file(SOCKET_A);
    let _ = remove_file(SOCKET_B);

    let side_a = UdSocket::bind(SOCKET_A).expect("side A bind failed");
    let side_b = UdSocket::bind(SOCKET_B).expect("side B bind failed");

    let child_thread = thread::spawn(move || {
        block_in_new_rt(inner(&side_b, "B", "A", SOCKET_A));
        let _ = remove_file(SOCKET_B);
    });

    block_in_new_rt(inner(&side_a, "A", "B", SOCKET_B));
    child_thread.join().expect("side B thread panicked");
    let _ = remove_file(SOCKET_A);
}

async fn inner(socket: &UdSocket, side: &str, other_side: &str, other_side_path: &str) {
    macro_rules! texpectcl {
        ($msg:literal) => {
            |e| panic!(concat!($msg, " for side {}: {:?}"), side, e)
        };
    }
    socket
        .set_destination(other_side_path)
        .unwrap_or_else(texpectcl!("setting destination failed"));

    let outgoing_message = format!("Hello from side {}!", side);

    let write = async {
        socket
            .send(outgoing_message.as_bytes())
            .await
            .unwrap_or_else(texpectcl!("write failed"))
    };

    let mut buffer = vec![0; 2048];
    let mut readbuf = ReadBuf::new(&mut buffer);
    let read = async {
        socket
            .recv(&mut readbuf)
            .await
            .unwrap_or_else(texpectcl!("read failed"));
    };

    join!(write, read);
    drop((socket, outgoing_message));

    let decoded_message = String::from_utf8_lossy(&buffer);
    let expected_message = format!("Hello from side {}!", other_side);

    assert_eq!(decoded_message, expected_message);
}
