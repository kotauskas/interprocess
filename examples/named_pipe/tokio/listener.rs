//{
#[cfg(not(all(windows, feature = "tokio")))]
fn main() {
    #[rustfmt::skip] eprintln!("\
This example is not available on platforms other than Windows or when the \
Tokio feature is disabled.");
}
#[cfg(all(windows, feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //}
    use {
        interprocess::os::windows::named_pipe::{pipe_mode, tokio::*, PipeListenerOptions},
        std::{io, path::Path},
        tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            try_join,
        },
    };

    // Describe the things we do when we've got a connection ready.
    async fn handle_conn(conn: DuplexPipeStream<pipe_mode::Bytes>) -> io::Result<()> {
        let [mut recver, mut sender] = [&conn; 2];
        // Allocate a small buffer for receiving.
        let mut buffer = String::with_capacity(128);

        // Describe the send operation as first sending our whole message,
        // and then shutting down the send half to make sure Tokio flushes
        // its internal buffer.
        let send = async {
            sender.write_all(b"Hello from server!").await?;
            sender.shutdown().await?;
            Ok(())
        };

        // Describe the receive operation as receiving into our buffer.
        let recv = recver.read_to_string(&mut buffer);

        // Run both the send-and-invoke-EOF operation
        // and the receive operation concurrently.
        try_join!(recv, send)?;

        // Close the connection right away to avoid holding up resources.
        drop(conn);

        // Print the result!
        println!("Client answered: {}", buffer.trim());
        Ok(())
    }

    const PIPE_NAME: &str = "Example";

    // Create our listener.
    let listener = PipeListenerOptions::new()
        .path(Path::new(PIPE_NAME))
        .create_tokio_duplex::<pipe_mode::Bytes>()?;

    // This is a good place to inform clients that the server is ready.
    eprintln!(r"Server running at \\.\pipe\{PIPE_NAME}");

    // Set up our loop boilerplate that processes our incoming connections.
    loop {
        // Sort out situations when establishing an incoming connection caused an error.
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("There was an error with an incoming connection: {e}");
                continue;
            }
        };

        // Spawn new parallel asynchronous tasks onto the Tokio runtime and hand the connection
        // over to them so that multiple clients could be processed simultaneously in a
        // lightweight fashion.
        tokio::spawn(async move {
            // The outer match processes errors that happen when we're connecting to something.
            // The inner if-let processes errors that happen during the connection.
            if let Err(e) = handle_conn(conn).await {
                eprintln!("error while handling connection: {e}");
            }
        });
    }
} //
