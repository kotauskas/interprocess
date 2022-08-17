use interprocess::local_socket::LocalSocketStream;
use std::io::{self, prelude::*, BufReader};

pub fn main() -> anyhow::Result<()> {
    let mut conn = BufReader::new(LocalSocketStream::connect("/tmp/teletype.sock")?);
    eprintln!("Teletype client connected to server.");
    let mut our_turn = true;
    let mut buffer = String::new();
    loop {
        if our_turn {
            io::stdin().read_line(&mut buffer)?;
            conn.get_mut().write_all(buffer.as_ref())?;
        } else {
            conn.read_line(&mut buffer)?;
            io::stdout().write_all(buffer.as_ref())?;
        }
        buffer.clear();
        our_turn = !our_turn;
    }
}
