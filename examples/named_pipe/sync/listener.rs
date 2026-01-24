//{
#[cfg(not(windows))]
fn main() {
    eprintln!("This example is not available on platforms other than Windows.");
}
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    //}
    use {
        interprocess::os::windows::named_pipe::{
            pipe_mode, DuplexPipeStream, PipeListenerOptions,
        },
        std::{
            io::{self, prelude::*, BufReader},
            path::Path,
        },
    };

    type Stream = DuplexPipeStream<pipe_mode::Bytes>;

    // Define a function that checks for errors in incoming connections. We'll use this to filter
    // through connections that fail on initialization for one reason or another.
    fn handle_error(conn: io::Result<Stream>) -> Option<Stream> {
        match conn {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {e}");
                None
            }
        }
    }

    const PIPE_NAME: &str = "Example";

    // Create our listener.
    let listener = PipeListenerOptions::new()
        .path(Path::new(PIPE_NAME))
        .create_duplex::<pipe_mode::Bytes>()?;

    // The syncronization between the server and client, if any is used, goes here.
    eprintln!(r"Server running at \\.\pipe\{PIPE_NAME}");

    // Preemptively allocate a sizeable buffer for receiving at a later moment. This size should
    // be enough and should be easy to find for the allocator. Since we only have one concurrent
    // client, there's no need to reallocate the buffer repeatedly.
    let mut buffer = String::with_capacity(128);

    for conn in listener.incoming().filter_map(handle_error) {
        // Wrap the connection into a buffered receiver right away
        // so that we could receive a single line from it.
        let mut conn = BufReader::new(conn);
        println!("Incoming connection!");

        // Since our client example sends first, the server should receive a line and only then
        // send a response. Otherwise, because receiving from and sending to a connection cannot
        // be simultaneous without threads or async, we can deadlock the two processes by having
        // both sides wait for the send buffer to be emptied by the other.
        conn.read_line(&mut buffer)?;

        // Now that the receive has come through and the client is waiting on the server's send,
        // do it. (`.get_mut()` is to get the sender, `BufReader` doesn't implement a
        // pass-through `Write`.)
        conn.get_mut().write_all(b"Hello from server!\n")?;

        // Print out the result, getting the newline for free!
        print!("Client answered: {buffer}");

        // Clear the buffer so that the next iteration will display new data instead of messages
        // stacking on top of one another.
        buffer.clear();
    }
    //{
    Ok(())
} //}
