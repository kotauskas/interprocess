use std::io;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};

fn main() {
    fn handle_error(
        connection: io::Result<LocalSocketStream>,
    ) -> LocalSocketStream {
        match connection {
            Ok(val) => val,
            Err(error) => {
                eprintln!("\n");
                panic!("Incoming connection failed: {}", error);
            }
        }
    }

    let listener = LocalSocketListener::bind("/tmp/teletype.sock").expect("failed to set up server");
    eprintln!("Teletype server listening for connections.");
    for mut conn in listener.incoming().map(handle_error) {
        println!("\n");
        io::copy(&mut conn, &mut io::stdout()).expect("failed to copy from socket to stdout");
    }
}