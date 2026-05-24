use std::env;
use std::net::{SocketAddr, TcpStream};
use std::process;
use std::time::Duration;

fn is_port_busy(ip: &str, port: u16) -> bool {
    let addr: SocketAddr = format!("{}:{}", ip, port)
        .parse()
        .expect(&format!("Failed to parse address {}:{}", ip, port));

    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage:\n {} <IP> <START_PORT> <END_PORT>", args[0]);
        process::exit(1);
    }

    let ip = &args[1];
    let start_port: u16 = args[2].parse().unwrap();
    let end_port: u16 = args[3].parse().unwrap();

    if start_port > end_port {
        eprintln!("Error: START_PORT > END_PORT");
        process::exit(1);
    }

    println!(
        "Start check ports from {} to {} on {}",
        start_port, end_port, ip
    );

    for port in start_port..=end_port {
        if is_port_busy(ip, port) {
            println!("Port {} is busy", port);
        }
    }
}
