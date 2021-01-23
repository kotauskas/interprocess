use futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use interprocess::nonblocking::local_socket::*;

#[tokio::main]
async fn main() {
    // Replace the path as necessary on Windows.
    let mut conn = LocalSocketStream::connect("/tmp/example.sock")
        .await
        .unwrap();
    conn.write_all(b"Hello from client!\n").await.unwrap();
    let mut conn = BufReader::new(conn);
    let mut buffer = String::new();
    conn.read_line(&mut buffer).await.unwrap();
    println!("Server answered: {}", buffer);
}
