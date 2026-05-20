use lettre::{
    SmtpTransport, Transport,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::env;

static SENDER: &str = "pyaterka20@gmail.com";
static SMTP_SERVER: &str = "smtp.gmail.com";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <recipient> <format> <body>", args[0]);
        std::process::exit(1);
    }

    let recipient = &args[1];
    let format = &args[2];
    let body = &args[3];

    let content_type = match format.as_str() {
        "txt" => ContentType::TEXT_PLAIN,
        "html" => ContentType::TEXT_HTML,
        _ => {
            eprintln!("Unsupported format: use txt or html");
            std::process::exit(1);
        }
    };

    let email = Message::builder()
        .from(SENDER.parse::<Mailbox>()?)
        .to(recipient.parse::<Mailbox>()?)
        .subject("Test email")
        .header(content_type)
        .body(body.to_string())?;

    let creds = Credentials::new(SENDER.to_string(), std::env::var("PASSWORD")?);
    let mailer = SmtpTransport::starttls_relay(SMTP_SERVER)?
        .credentials(creds)
        .build();

    match mailer.send(&email) {
        Ok(_) => println!("Email sent successfully"),
        Err(e) => eprintln!("Could not send email: {e}"),
    }

    Ok(())
}
