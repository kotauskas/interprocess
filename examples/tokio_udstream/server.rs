use interprocess::os::unix::udsocket::tokio::{UdStream, UdStreamListener};
use std::io;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    try_join,
    sync::oneshot::Sender,
};

pub async fn main(notify: Sender<()>) -> anyhow::Result<()> {
    // Describe the things we do when we've got a connection ready.
    async fn handle_conn(mut conn: UdStream) -> io::Result<()> {
        // Split the connection into two halves to process
        // received and sent data concurrently.
        let (mut reader, mut writer) = conn.split();

        // Allocate a sizeable buffer for reading.
        // This size should be enough and should be easy to find for the allocator.
        let mut buffer = String::with_capacity(128);

        // Describe the write operation as first writing our whole message, and
        // then shutting down the write half to send an EOF to help the other
        // side determine the end of the transmission.
        let write = async {
            writer.write_all(b"Hello from server!").await?;
            writer.shutdown()?;
            Ok(())
        };

        // Describe the read operation as reading into our big buffer.
        let read = reader.read_to_string(&mut buffer);

        // Run both the write-and-send-EOF operation and the read operation concurrently.
        try_join!(read, write)?;

        // Dispose of our connection right now and not a moment later because I want to!
        drop(conn);

        // Produce our output!
        println!("Client answered: {}", buffer.trim());
        Ok(())
    }

    static SOCKET_PATH: &str = "/tmp/example.sock";

    // Create our listener. In a more robust program, we'd check for an
    // existing socket file that has not been deleted for whatever reason,
    // ensure it's a socket file and not a normal file, and delete it.
    let listener = UdStreamListener::bind(SOCKET_PATH)?;
    // Stand-in for the syncronization used, if any, between the client and the server.
    let _ = notify.send(());
    println!("Server running at {}", SOCKET_PATH);

    // Set up our loop boilerplate that processes our incoming connections.
    loop {
        // Sort out situations when establishing an incoming connection caused an error.
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("There was an error with an incoming connection: {}", e);
                continue;
            }
        };

        // Spawn new parallel asynchronous tasks onto the Tokio runtime
        // and hand the connection over to them so that multiple clients
        // could be processed simultaneously in a lightweight fashion.
        tokio::spawn(async move {
            // The outer match processes errors that happen when we're
            // connecting to something. The inner if-let processes errors that
            // happen during the connection.
            if let Err(e) = handle_conn(conn).await {
                eprintln!("error while handling connection: {}", e);
            }
        });
    }
}
