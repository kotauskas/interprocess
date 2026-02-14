//{
#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example is not available when the Tokio feature is disabled.");
}
#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //}
    use {
        interprocess::local_socket::{
            tokio::{prelude::*, Stream},
            GenericFilePath, GenericNamespaced,
        },
        tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            try_join,
        },
    };

    // Pick a name.
    let name = if GenericNamespaced::is_supported() {
        "example.sock".to_ns_name::<GenericNamespaced>()?
    } else {
        "/tmp/example.sock".to_fs_name::<GenericFilePath>()?
    };

    // Await this here since we can't do a whole lot without a connection.
    let conn = Stream::connect(name).await?;

    // Create a buffered reader that wraps the connection by reference
    // so we can receive a single line.
    let mut recver = BufReader::new(&conn);
    // The "sender" will just be a shared reference to the connection,
    // allowing us to read and write concurrently. It is okay to ditch
    // this reference and reborrow it at any time to satisfy the borrow
    // check.
    let mut sender = &conn;

    // Allocate a small result buffer for receiving.
    let mut buffer = String::with_capacity(128);

    // Describe the send operation as writing
    // our whole string into the connection.
    let send = sender.write_all(b"Hello from client!\n");
    // Describe the receive operation as receiving until a newline
    // from our buffered reader into our buffer.
    let recv = recver.read_line(&mut buffer);

    // Concurrently perform both operations.
    try_join!(send, recv)?;

    // Deallocate the read buffer and close the connection right away
    // to avoid holding up resources.
    drop(recver);
    drop(conn);

    // Display the result!
    println!("Server answered: {}", buffer.trim());
    //{
    Ok(())
} //}
