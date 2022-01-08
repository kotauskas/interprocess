use futures::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    try_join,
};
use interprocess::os::windows::named_pipe::tokio::*;
use std::error::Error;

pub async fn main() -> Result<(), Box<dyn Error>> {
    let conn = DuplexBytePipeStream::connect("Example")?;
    let (reader, mut writer) = conn.split();
    let mut reader = BufReader::new(reader);
    let mut buffer = String::new();
    let write = writer.write_all(b"Hello from client!\n");
    let read = reader.read_line(&mut buffer);
    try_join!(write, read)?;
    println!("Server answered: {}", buffer.trim());
    Ok(())
}
