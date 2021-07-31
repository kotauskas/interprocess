use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::io::{self, prelude::*, BufReader};

fn main() {
    fn handle_error(connection: io::Result<LocalSocketStream>) -> LocalSocketStream {
        match connection {
            Ok(val) => val,
            Err(error) => {
                eprintln!("\n");
                panic!("Incoming connection failed: {}", error);
            }
        }
    }

    let listener =
        LocalSocketListener::bind("/tmp/teletype.sock").expect("failed to set up server");
    eprintln!("Teletype server listening for connections.");
    let mut conn = listener
        .incoming()
        .next()
        .map(handle_error)
        .map(BufReader::new)
        .unwrap();
    let mut our_turn = false;
    let mut buffer = String::new();
    loop {
        if our_turn {
            io::stdin()
                .read_line(&mut buffer)
                .expect("failed to read line from stdin");
            conn.get_mut()
                .write_all(buffer.as_ref())
                .expect("failed to write line to socket");
        } else {
            conn.read_line(&mut buffer)
                .expect("failed to read line from socket");
            io::stdout()
                .write_all(buffer.as_ref())
                .expect("failed to write line to stdout");
        }
        buffer.clear();
        our_turn = !our_turn;
    }
}
