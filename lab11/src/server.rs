use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv6Addr, SocketAddrV6, TcpListener, TcpStream};

fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let peer = stream.peer_addr()?;
    println!("Client connected: {}", peer);

    let mut reader = BufReader::new(stream.try_clone()?);
    loop {
        let mut message = String::new();
        let bytes_read = reader.read_line(&mut message)?;

        if bytes_read == 0 {
            println!("Client disconnected: {}", peer);
            break;
        }

        let response = message.trim_end().to_uppercase();

        println!("Received: {}", message.trim_end());
        println!("Sending: {}", response);

        writeln!(stream, "{}", response)?;
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    let address = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 8080, 0, 0);
    let listener = TcpListener::bind(address)?;

    println!("IPv6 TCP Echo Server listening on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_client(stream) {
                    eprintln!("Client error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}
