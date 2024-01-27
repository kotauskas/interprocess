use std::io::{self, Read, Write};
use interprocess::local_socket::LocalSocketStream;

fn main() -> io::Result<()> {
    // let mut stream = LocalSocketStream::connect(sharing::service::CHANNEL_NAME)?;
    let mut stream = LocalSocketStream::connect("test-jorge")?;
    loop {
        println!("Choose an option:");
        println!("1. Start web-driver");
        println!("2. Stop web-driver");
        println!("3. Restart web-driver");
        println!("0. Exit");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let option: u32 = match input.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("Invalid input. Please choose a valid option.");
                continue;
            }
        };

        match option {
            1 => send_command(&mut stream, "0;true")?, // Start web-driver
            2 => send_command(&mut stream, "1;true")?, // Stop web-driver
            3 => send_command(&mut stream, "2;true")?, // Restart web-driver
            0 => {
                send_command(&mut stream, "4;true")?; // Exit
                println!("Exiting...");
                break;
            }
            _ => println!("Invalid option. Please choose a valid option."),
        }
    }
    Ok(())
}

fn send_command(stream: &mut LocalSocketStream, message: &str) -> io::Result<()> {
    println!("Sending message: {}", message);
    stream.write_all(message.as_bytes())?;
    stream.flush()?;

    let mut buffer = Vec::new(); // Use a dynamic Vec to store the incoming message
    let mut partial_buffer = [0; 1024];
    match stream.read(&mut partial_buffer) {
        Ok(size) => {
            if size == 0 {
                println!("Connection closed by server");
            }
            buffer.extend_from_slice(&partial_buffer[..size]);
            if size < partial_buffer.len() {
                println!("Message received: {}", String::from_utf8_lossy(&buffer));
            }
        }
        Err(e) => {
            eprintln!("Failed to read from socket in dedicated thread {}", e);
        }
    }

    let message = String::from_utf8_lossy(&buffer).to_string();
    println!("Message received: {}", message);
    Ok(())
}