use futures::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    try_join,
};
use interprocess::os::windows::named_pipe::{tokio::*, PipeListenerOptions};
use std::{error::Error, ffi::OsStr};

pub async fn main() -> Result<(), Box<dyn Error>> {
    let listener = PipeListenerOptions::new()
        .name(OsStr::new("Example"))
        .create_tokio::<DuplexBytePipeStream>()?;
    loop {
        let conn = listener.accept().await?;
        let (reader, mut writer) = conn.split();
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        let write = writer.write_all(b"Hello from server!\n");
        let read = reader.read_line(&mut buffer);
        try_join!(read, write)?;
        println!("Client answered: {}", buffer.trim());
    }
}
