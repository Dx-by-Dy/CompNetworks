use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv6Addr, SocketAddrV6, TcpStream};

fn main() -> std::io::Result<()> {
    let server_addr = SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 0, 0);
    let mut stream = TcpStream::connect(server_addr)?;

    println!("Connected to server");

    let mut message = std::env::args()
        .nth(1)
        .unwrap_or("default message".to_string());
    message = message.trim_end().to_string();
    message.push('\n');

    println!("Sending: {}", message.trim_end());

    stream.write_all(message.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    println!("Server response: {}", response.trim_end());

    Ok(())
}
