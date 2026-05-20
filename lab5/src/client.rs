use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <server_ip> <command>",
            args[0]
        );

        std::process::exit(1);
    }

    let server_ip = &args[1];
    let command = &args[2];
    let address = format!("{server_ip}:5555");
    let mut stream = TcpStream::connect(address)?;
    stream.write_all(command.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    println!("Server response:\n{}", response);

    Ok(())
}