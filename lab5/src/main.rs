use std::{
    env,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

fn read_response(reader: &mut BufReader<TcpStream>) -> std::io::Result<String> {
    let mut response = String::new();
    reader.read_line(&mut response)?;

    print!("SERVER: {}", response);
    Ok(response)
}

fn send_command(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    command: &str,
) -> std::io::Result<()> {
    print!("CLIENT: {}", command);

    stream.write_all(command.as_bytes())?;
    stream.flush()?;
    read_response(reader)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 {
        eprintln!("Usage: {} <server> <port> <from> <to> <message>", args[0]);
        std::process::exit(1);
    }

    let server = &args[1];
    let port = &args[2];
    let from = &args[3];
    let to = &args[4];
    let message = &args[5];

    let address = format!("{server}:{port}");
    let mut stream = TcpStream::connect(address)?;
    let mut reader = BufReader::new(stream.try_clone()?);

    read_response(&mut reader)?;

    send_command(&mut stream, &mut reader, "HELO localhost\r\n")?;
    send_command(
        &mut stream,
        &mut reader,
        &format!("MAIL FROM:<{}>\r\n", from),
    )?;
    send_command(&mut stream, &mut reader, &format!("RCPT TO:<{}>\r\n", to))?;
    send_command(&mut stream, &mut reader, "DATA\r\n")?;

    let data = format!(
        "Subject: Test message\r\n\
         From: {}\r\n\
         To: {}\r\n\
         Content-Type: text/plain; charset=UTF-8\r\n\
         \r\n\
         {}\r\n\
         .\r\n",
        from, to, message
    );
    print!("CLIENT: {}", data);

    stream.write_all(data.as_bytes())?;
    stream.flush()?;
    read_response(&mut reader)?;

    send_command(&mut stream, &mut reader, "QUIT\r\n")?;

    Ok(())
}
