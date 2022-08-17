use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{
    io::{self, prelude::*, BufReader},
    sync::mpsc::Sender,
};

pub fn main(notify: Sender<()>) -> anyhow::Result<()> {
    fn handle_error(connection: io::Result<LocalSocketStream>) -> LocalSocketStream {
        match connection {
            Ok(val) => val,
            Err(error) => {
                eprintln!("\n");
                panic!("Incoming connection failed: {}", error);
            }
        }
    }

    let listener = LocalSocketListener::bind("/tmp/teletype.sock")?;
    // Stand-in for the syncronization used, if any, between the client and the server.
    let _ = notify.send(());
    eprintln!("Teletype server listening for connections.");
    let mut conn = listener
        .incoming()
        .next()
        .map(handle_error)
        .map(BufReader::new)
        .unwrap();
    let mut our_turn = false;
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
