use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{
    error::Error,
    io::{self, prelude::*, BufReader},
};

fn main() -> Result<(), Box<dyn Error>> {
    fn handle_error(connection: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
        connection
            .map_err(|error| eprintln!("Incoming connection failed: {}", error))
            .ok()
    }

    let listener = LocalSocketListener::bind("/tmp/example.sock")?;
    for mut conn in listener.incoming().filter_map(handle_error) {
        println!("Incoming connection!");

        conn.write_all(b"Hello from server!\n")?;

        // Add buffering to the connection to read a line.
        let mut conn = BufReader::new(conn);
        let mut buffer = String::new();
        conn.read_line(&mut buffer)?;

        println!("Client answered: {}", buffer);
    }
    Ok(())
}
