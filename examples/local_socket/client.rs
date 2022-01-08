use interprocess::local_socket::LocalSocketStream;
use std::{
    error::Error,
    io::{prelude::*, BufReader},
};

pub fn main() -> Result<(), Box<dyn Error>> {
    let mut conn = LocalSocketStream::connect("/tmp/example.sock")?;
    conn.write_all(b"Hello from client!\n")?;

    let mut conn = BufReader::new(conn);
    let mut buffer = String::new();
    conn.read_line(&mut buffer)?;

    println!("Server answered: {}", buffer);

    Ok(())
}
