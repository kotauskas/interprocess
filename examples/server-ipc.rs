use std::sync::Arc;
use parking_lot::RwLock;
use interprocess::local_socket::{LocalSocketListener};
use std::io::Read;
use std::io::Write;
// use interprocess::unnamed_pipe::pipe;
fn main() {
    // let listener = LocalSocketListener::bind_unsafe(sharing::service::CHANNEL_NAME)
    let listener = LocalSocketListener::bind_unsafe("test-jorge")
        .expect("Failed to bind");
    println!("Bind successful secure");
    // let (mut reader, mut writer) = pipe().unwrap();


    let listener: Arc<RwLock<LocalSocketListener>> = Arc::new(RwLock::new(listener));

    loop {
        for connection in listener.write().incoming() {
            match connection {
                Ok(mut stream) => {
                    println!("New connection");
                    loop {
                        // Read the incoming message.
                        let mut buffer = [0; 1024];
                        let bytes_read = stream.read(&mut buffer).unwrap();
                        let received_message = String::from_utf8_lossy(&buffer[..bytes_read]);
                        println!("Received message: {}", received_message);
                        if received_message == "4;true" {
                            println!("Exiting...");
                            break;
                        }

                        // Echo the received message back to the client.
                        let response = format!("You said: {}", received_message);
                        if let Err(e) = stream.write_all(response.as_bytes()) {
                            println!("Failed to write response: {:?}", e);
                            break;
                        }
                    }

                }
                Err(_) => {
                }
            }
        }
    }
}
