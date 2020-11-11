use futures::io::{AsyncReadExt, AsyncWriteExt};
use interprocess::nonblocking::local_socket::*;

#[tokio::main]
async fn main() {
    // Replace the path as necessary on Windows.
    let mut conn = LocalSocketStream::connect("/tmp/example.sock")
        .await
        .unwrap();
    conn.write_all(b"Hello from client!").await.unwrap();
    let mut buffer = String::new();
    conn.read_to_string(&mut buffer).await.unwrap();
    println!("Server answered: {}", buffer);
}
