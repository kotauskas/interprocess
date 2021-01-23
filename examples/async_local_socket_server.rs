use futures::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    stream::TryStreamExt,
};
use interprocess::nonblocking::local_socket::*;

#[tokio::main]
async fn main() {
    let listener = LocalSocketListener::bind("/tmp/example.sock")
        .await
        .unwrap();
    listener
        .incoming()
        .try_for_each(|mut conn| async move {
            conn.write_all(b"Hello from server!\n").await.unwrap();
            let mut conn = BufReader::new(conn);
            let mut buffer = String::new();
            conn.read_line(&mut buffer).await.unwrap();
            println!("Client answered: {}", buffer);
            Ok(())
        })
        .await
        .unwrap();
}
