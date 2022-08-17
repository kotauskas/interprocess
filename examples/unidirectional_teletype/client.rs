use interprocess::local_socket::LocalSocketStream;
use std::io;

pub fn main() -> anyhow::Result<()> {
    let mut stream = LocalSocketStream::connect("/tmp/teletype.sock")?;
    eprintln!("Teletype client connected to server.\n");
    io::copy(&mut io::stdin(), &mut stream)?;
    Ok(())
}
