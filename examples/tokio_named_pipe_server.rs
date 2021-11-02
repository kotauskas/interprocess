#[cfg(windows)]
#[tokio::main]
async fn main() {
    use futures::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        try_join,
    };
    use interprocess::os::windows::named_pipe::{tokio::*, PipeListenerOptions};
    use std::ffi::OsStr;

    let listener = PipeListenerOptions::new()
        .name(OsStr::new("Example"))
        .create_tokio::<DuplexBytePipeStream>()
        .unwrap();
    loop {
        let conn = listener.accept().await.unwrap();
        let (reader, mut writer) = conn.split();
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        let write = writer.write_all(b"Hello from server!\n");
        let read = reader.read_line(&mut buffer);
        try_join!(read, write).unwrap();
        println!("Client answered: {}", buffer.trim());
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("uh oh, you're not on Windows");
}
