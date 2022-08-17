use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use std::{io, sync::mpsc::Sender};

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
    let _ = notify.send(());
    eprintln!("Teletype server listening for connections.");
    for mut conn in listener.incoming().map(handle_error) {
        println!("\n");
        io::copy(&mut conn, &mut io::stdout())?;
    }
    unreachable!()
}
