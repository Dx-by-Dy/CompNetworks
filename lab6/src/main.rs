use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;

struct FtpClient {
    control: TcpStream,
}

impl FtpClient {
    fn connect(addr: &str) -> std::io::Result<Self> {
        let mut stream = TcpStream::connect(addr)?;
        let response = Self::read_response(&mut stream)?;

        println!("SERVER: {}", response.trim());
        Ok(Self { control: stream })
    }

    fn login(&mut self, user: &str, pass: &str) -> std::io::Result<()> {
        self.send_command(&format!("USER {}\r\n", user))?;
        println!("{}", self.read()?);

        self.send_command(&format!("PASS {}\r\n", pass))?;
        println!("{}", self.read()?);
        Ok(())
    }

    fn send_command(&mut self, cmd: &str) -> std::io::Result<()> {
        print!("CLIENT: {}", cmd);
        self.control.write_all(cmd.as_bytes())?;
        Ok(())
    }

    fn read(&mut self) -> std::io::Result<String> {
        Self::read_response(&mut self.control)
    }

    fn read_response(stream: &mut TcpStream) -> std::io::Result<String> {
        let mut buffer = [0u8; 4096];
        let size = stream.read(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer[..size]).to_string())
    }

    fn enter_passive_mode(&mut self) -> std::io::Result<TcpStream> {
        self.send_command("PASV\r\n")?;
        let response = self.read()?;

        println!("PASV RESPONSE: {}", response.trim());

        let start = response.find('(').unwrap();
        let end = response.find(')').unwrap();
        let numbers: Vec<u16> = response[start + 1..end]
            .split(',')
            .map(|s| s.parse::<u16>().unwrap())
            .collect();
        let ip = format!(
            "{}.{}.{}.{}",
            numbers[0], numbers[1], numbers[2], numbers[3]
        );
        let port = numbers[4] * 256 + numbers[5];

        println!("DATA CONNECTION: {}:{}", ip, port);

        TcpStream::connect(format!("{}:{}", ip, port))
    }

    fn list(&mut self) -> std::io::Result<()> {
        let mut data_stream = self.enter_passive_mode()?;
        self.send_command("LIST\r\n")?;

        println!("{}", self.read()?);

        let mut data = String::new();
        data_stream.read_to_string(&mut data)?;

        println!("FILES:\n{}", data);
        println!("{}", self.read()?);

        Ok(())
    }

    fn download_file(&mut self, remote_file: &str, local_file: &str) -> std::io::Result<()> {
        let mut data_stream = self.enter_passive_mode()?;
        self.send_command(&format!("RETR {}\r\n", remote_file))?;

        println!("{}", self.read()?);

        let mut file = File::create(local_file)?;
        let mut buffer = [0u8; 4096];
        loop {
            let n = data_stream.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])?;
        }

        println!("Downloaded {}", local_file);
        println!("{}", self.read()?);

        Ok(())
    }

    fn upload_file(&mut self, local_file: &str, remote_file: &str) -> std::io::Result<()> {
        let mut data_stream = self.enter_passive_mode()?;
        self.send_command(&format!("STOR {}\r\n", remote_file))?;

        println!("{}", self.read()?);

        let mut file = File::open(local_file)?;
        let mut buffer = [0u8; 4096];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            data_stream.write_all(&buffer[..n])?;
        }

        println!("Uploaded {}", local_file);
        println!("{}", self.read()?);

        Ok(())
    }

    fn quit(&mut self) -> std::io::Result<()> {
        self.send_command("QUIT\r\n")?;
        println!("{}", self.read()?);
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
    let mut ftp = FtpClient::connect("44.241.66.173:21")?;

    ftp.login("dlpuser", "rNrKYTX9g7z3RgJRmxWuGHbeu")?;
    ftp.list()?;

    println!("Enter remote file name to download:");
    let mut remote_file = String::new();
    std::io::stdin().read_line(&mut remote_file)?;

    println!("Enter local file name to save:");
    let mut local_file = String::new();
    std::io::stdin().read_line(&mut local_file)?;

    ftp.download_file(&remote_file.trim(), &local_file.trim())?;

    println!("Enter local file name to upload:");
    let mut local_file = String::new();
    std::io::stdin().read_line(&mut local_file)?;

    println!("Enter remote file name to upload:");
    let mut remote_file = String::new();
    std::io::stdin().read_line(&mut remote_file)?;

    ftp.upload_file(&local_file.trim(), &remote_file.trim())?;

    ftp.quit()?;
    Ok(())
}
