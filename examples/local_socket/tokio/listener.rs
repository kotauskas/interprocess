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
            GenericNamespaced, ListenerOptions,
        },
        std::io,
        tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            try_join,
        },
    };

    // Describe the things we do when we've got a connection ready.
    async fn handle_conn(conn: Stream) -> io::Result<()> {
        let mut recver = BufReader::new(&conn);
        let mut sender = &conn;

        // Allocate a small buffer for receiving.
        let mut buffer = String::with_capacity(128);

        // Describe the send operation as sending our whole message.
        let send = sender.write_all(b"Hello from server!\n");
        // Describe the receive operation as receiving a line into our buffer.
        let recv = recver.read_line(&mut buffer);

        // Run both operations concurrently.
        try_join!(recv, send)?;

        // Print the result!
        println!("Client answered: {}", buffer.trim());
        Ok(())
    }

    // Pick a name.
    let printname = "example.sock";
    let name = printname.to_ns_name::<GenericNamespaced>()?;

    // Configure our listener...
    let opts = ListenerOptions::new().name(name);

    // ...and create it.
    let listener = match opts.create_tokio() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            // When a program that uses a file-type socket name terminates its socket server
            // without deleting the file, a "corpse socket" remains, which can neither be
            // connected to nor reused by a new listener. Normally, Interprocess takes care of
            // this on affected platforms by deleting the socket file when the listener is
            // dropped. (This is vulnerable to all sorts of races and thus can be disabled.)
            //
            // In a real program, instead of leaving it up to the user to perform cleanup, one
            // would use the .try_overwrite(true) listener option to try to replace the socket.
            eprintln!(
                "Error: could not start server because the socket file is occupied. Please check
                if {printname} is in use by another process and try again."
            );
            return Err(e.into());
        }
        x => x?,
    };

    // This is a good place to inform clients that the server is ready.
    eprintln!("Server running at {printname}");

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
                eprintln!("Error while handling connection: {e}");
            }
        });
    }
} //
