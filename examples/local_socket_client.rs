use interprocess::local_socket::LocalSocketStream;
use std::{
    io::{prelude::*, BufReader},
    error::Error,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut conn = LocalSocketStream::connect("/tmp/example.sock").unwrap();
    conn.write_all(b"Hello from client!\n").unwrap();

    let mut conn = BufReader::new(conn);
    let mut buffer = String::new();
    conn.read_line(&mut buffer).unwrap();

    println!("Server answered: {}", buffer);

    Ok(())
}