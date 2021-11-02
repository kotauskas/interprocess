#[cfg(windows)]
#[tokio::main]
async fn main() {
    use futures::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        try_join,
    };
    use interprocess::os::windows::named_pipe::tokio::*;

    let conn = DuplexBytePipeStream::connect("Example").unwrap();
    let (reader, mut writer) = conn.split();
    let mut reader = BufReader::new(reader);
    let mut buffer = String::new();
    let write = writer.write_all(b"Hello from client!\n");
    let read = reader.read_line(&mut buffer);
    try_join!(write, read).unwrap();
    println!("Server answered: {}", buffer.trim());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("uh oh, you're not on Windows");
}
