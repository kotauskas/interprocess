use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{error::Error, io};

pub fn main() -> Result<(), Box<dyn Error>> {
    fn handle_error(connection: io::Result<LocalSocketStream>) -> LocalSocketStream {
        match connection {
            Ok(val) => val,
            Err(error) => {
                eprintln!("\n");
                panic!("Incoming connection failed: {}", error);
            }
        }
    }

    let listener = LocalSocketListener::bind("/tmp/teletype.sock")?;
    eprintln!("Teletype server listening for connections.");
    for mut conn in listener.incoming().map(handle_error) {
        println!("\n");
        io::copy(&mut conn, &mut io::stdout())?;
    }
    unreachable!()
}
