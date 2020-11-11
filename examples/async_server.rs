use futures::{
    io::{AsyncReadExt, AsyncWriteExt},
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
        .try_for_each(|mut stream| async move {
            stream.write_all(b"Hello from server!").await.unwrap();
            let mut buffer = String::new();
            stream.read_to_string(&mut buffer).await.unwrap();
            println!("Client answered: {}", buffer);
            Ok(())
        })
        .await
        .unwrap();
}
