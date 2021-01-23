use interprocess::local_socket::LocalSocketStream;
use std::io::{self, prelude::*, BufReader};

fn main() {
    let mut conn = BufReader::new(
        LocalSocketStream::connect("/tmp/teletype.sock").expect("failed to connect"),
    );
    eprintln!("Teletype client connected to server.");
    let mut our_turn = true;
    let mut buffer = String::new();
    loop {
        if our_turn {
            io::stdin()
                .read_line(&mut buffer)
                .expect("failed to read line from stdin");
            conn.get_mut()
                .write_all(buffer.as_ref())
                .expect("failed to write line to socket");
            buffer.clear();
        } else {
            conn.read_line(&mut buffer)
                .expect("failed to read line from socket");
            io::stdout()
                .write_all(buffer.as_ref())
                .expect("failed to write line to stdout");
            buffer.clear();
        }
        our_turn = !our_turn;
    }
}
