use interprocess::os::unix::udsocket::tokio::*;
use std::{io, mem::MaybeUninit};
use tokio::{io::ReadBuf, sync::oneshot::Sender, try_join};

pub async fn main(src: &str, dst: &str, notify: Option<Sender<()>>) -> io::Result<()> {
    let socket_path = format!("/tmp/{}", src);
    // Socket creation happens immediately, no futures here.
    let socket = UdSocket::bind(socket_path)?;
    if let Some(n) = notify {
        let _ = n.send(());
    }
    // So does destination assignment.
    socket.set_destination(dst)?;

    // Allocate a stack buffer for reading at a later moment.
    let mut buffer = [MaybeUninit::<u8>::uninit(); 128];
    let mut readbuf = ReadBuf::uninit(&mut buffer);

    let message = format!("Hello from {}!", src);

    // Describe the write operation, but don't run it yet.
    // We'll launch it concurrently with the read operation.
    let write = socket.send(message.as_bytes());

    // Describe the read operation, and also don't run it yet.
    let read = socket.recv(&mut readbuf);

    // Perform both operations concurrently: the write and the read.
    try_join!(write, read)?;

    // Clean up early. Good riddance!
    drop(socket);

    // Convert the data that's been read into a string. This checks for UTF-8
    // validity, and if invalid characters are found, a new buffer is
    // allocated to house a modified version of the received data, where
    // decoding errors are replaced with those diamond-shaped question mark
    // U+FFFD REPLACEMENT CHARACTER thingies: �.
    let received_string = String::from_utf8_lossy(readbuf.filled());

    println!("Server answered: {}", &received_string);

    Ok(())
}
