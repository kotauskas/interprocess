use interprocess::local_socket::LocalSocketStream;
use std::{error::Error, io};

pub fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = LocalSocketStream::connect("/tmp/teletype.sock")?;
    eprintln!("Teletype client connected to server.\n");
    io::copy(&mut io::stdin(), &mut stream)?;
    Ok(())
}
