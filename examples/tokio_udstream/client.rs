use interprocess::os::unix::udsocket::tokio::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    try_join,
};

pub async fn main() -> anyhow::Result<()> {
    // Await this here since we can't do a whole lot without a connection.
    let mut conn = UdStream::connect("/tmp/example.sock").await?;

    // This takes an exclusive borrow of our connection and splits it into two
    // halves, so that we could concurrently act on both. Take care not to use
    // the .split() method from the futures crate's AsyncReadExt.
    let (mut reader, mut writer) = conn.split();

    // Allocate a sizeable buffer for reading.
    // This size should be enough and should be easy to find for the allocator.
    let mut buffer = String::with_capacity(128);

    // Describe the write operation as writing our whole string, waiting for
    // that to complete, and then shutting down the write half, which sends
    // an EOF to the other end to help it determine where the message ends.
    let write = async {
        writer.write_all(b"Hello from client!\n").await?;
        writer.shutdown()?;
        Ok(())
    };

    // Describe the read operation as reading until EOF into our big buffer.
    let read = reader.read_to_string(&mut buffer);

    // Concurrently perform both operations: write-and-send-EOF and read.
    try_join!(write, read)?;

    // Close the connection a bit earlier than you'd think we would. Nice practice!
    drop(conn);

    // Display the results when we're done!
    println!("Server answered: {}", buffer.trim());

    Ok(())
}
