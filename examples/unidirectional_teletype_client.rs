use interprocess::local_socket::LocalSocketStream;
use std::io;

fn main() {
    let mut stream = LocalSocketStream::connect("/tmp/teletype.sock").expect("failed to connect");
    eprintln!("Teletype client connected to server.\n");
    io::copy(&mut io::stdin(), &mut stream).expect("error while copying from stdin to socket");
}
