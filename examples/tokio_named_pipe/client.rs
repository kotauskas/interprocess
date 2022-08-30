use futures::{
    io::{AsyncReadExt, AsyncWriteExt},
    try_join,
};
use interprocess::os::windows::named_pipe::tokio::*;
use std::error::Error;

pub async fn main() -> Result<(), Box<dyn Error>> {
    // Await this here since we can't do a whole lot without a connection.
    let conn = DuplexBytePipeStream::connect("Example")?;

    // This consumes our connection and splits it into two owned halves, so that we could
    // concurrently act on both. Take care not to use the .split() method from the futures crate's
    // AsyncReadExt.
    let (mut reader, mut writer) = conn.split();

    // Preemptively allocate a sizeable buffer for reading.
    // This size should be enough and should be easy to find for the allocator.
    let mut buffer = String::with_capacity(128);

    // Describe the write operation as writing our whole string, waiting for
    // that to complete, and then shutting down the write half, which sends
    // an EOF to the other end to help it determine where the message ends.
    let write = async {
        writer.write_all(b"Hello from client!").await?;
        // Because only the trait from futures is implemented for now, it's "close" instead of
        // "shutdown".
        writer.close().await?;
        Ok(())
    };

    // Describe the read operation as reading until EOF into our big buffer.
    let read = reader.read_to_string(&mut buffer);

    // Concurrently perform both operations: write-and-send-EOF and read.
    try_join!(write, read)?;

    // Get rid of those here to close the read half too.
    drop((reader, writer));

    // Display the results when we're done!
    println!("Server answered: {}", buffer.trim());
    Ok(())
}
