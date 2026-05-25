use anyhow::{Result, bail};
use clap::Parser;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const ICMP_ECHO_REQUEST: u8 = 8;
const ICMP_ECHO_REPLY: u8 = 0;
const ICMP_TIME_EXCEEDED: u8 = 11;

const MAX_HOPS: u32 = 30;

#[derive(Parser)]
struct Args {
    host: String,

    #[arg(short, long, default_value_t = 3)]
    probes: u32,
}

#[derive(Debug)]
struct ProbeResult {
    seq: u16,
    ip: Ipv4Addr,
    icmp_type: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IcmpHeader {
    icmp_type: u8,
    code: u8,
    checksum: u16,
    identifier: u16,
    sequence: u16,
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;

    for chunk in data.chunks(2) {
        let value = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            (chunk[0] as u16) << 8
        };
        sum += value as u32;
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn build_icmp_packet(seq: u16, pid: u16) -> Vec<u8> {
    let mut packet = vec![0u8; 8 + 32];
    let header = IcmpHeader {
        icmp_type: ICMP_ECHO_REQUEST,
        code: 0,
        checksum: 0,
        identifier: pid,
        sequence: seq,
    };

    unsafe {
        std::ptr::write(packet.as_mut_ptr() as *mut IcmpHeader, header);
    }

    let checksum = checksum(&packet);

    unsafe {
        (*(packet.as_mut_ptr() as *mut IcmpHeader)).checksum = checksum.to_be();
    }

    packet
}

fn resolve_host(host: &str) -> Result<Ipv4Addr> {
    let addr = (host, 0)
        .to_socket_addrs()?
        .find(|a| matches!(a.ip(), IpAddr::V4(_)))
        .ok_or_else(|| anyhow::anyhow!("No IPv4 address found"))?;

    match addr.ip() {
        IpAddr::V4(ip) => Ok(ip),
        _ => bail!("IPv6 not supported"),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let dest_ip = resolve_host(&args.host)?;
    let pid = std::process::id() as u16;

    println!("Tracing route to {}\n", dest_ip);

    let socket = Socket::new(
        Domain::IPV4,
        Type::from(libc::SOCK_RAW),
        Some(Protocol::ICMPV4),
    )?;
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_nonblocking(false)?;

    let (tx, rx) = mpsc::channel::<ProbeResult>();
    let recv_socket = socket.try_clone()?;
    thread::spawn(move || {
        loop {
            let mut buf = [MaybeUninit::<u8>::zeroed(); 2048];
            let Ok((size, addr)) = recv_socket.recv_from(&mut buf) else {
                continue;
            };

            let ip = match addr.as_socket_ipv4() {
                Some(v4) => *v4.ip(),
                None => continue,
            };

            if size < 20 {
                continue;
            }

            let ip_header_len = ((unsafe { buf[0].assume_init() } & 0x0f) * 4) as usize;
            if size < ip_header_len + 8 {
                continue;
            }
            let icmp_type = unsafe { buf[ip_header_len].assume_init() };

            match icmp_type {
                ICMP_ECHO_REPLY => {
                    let icmp_header: IcmpHeader = unsafe {
                        let ptr = buf[ip_header_len].as_ptr() as *const IcmpHeader;
                        std::ptr::read_unaligned(ptr)
                    };

                    if icmp_header.identifier != pid {
                        continue;
                    }

                    let seq = icmp_header.sequence;
                    let _ = tx.send(ProbeResult { seq, ip, icmp_type });
                }
                ICMP_TIME_EXCEEDED => {
                    let inner_ip_offset = ip_header_len + 8;

                    if size < inner_ip_offset + 20 {
                        continue;
                    }

                    let inner_ip_header_len =
                        ((unsafe { buf[inner_ip_offset].assume_init() } & 0x0f) * 4) as usize;
                    let inner_icmp_offset = inner_ip_offset + inner_ip_header_len;
                    if size < inner_icmp_offset + 8 {
                        continue;
                    }

                    let inner_icmp: IcmpHeader = unsafe {
                        let ptr = buf[inner_icmp_offset].as_ptr() as *const IcmpHeader;
                        std::ptr::read_unaligned(ptr)
                    };

                    if inner_icmp.identifier != pid {
                        continue;
                    }

                    let seq = inner_icmp.sequence;
                    let _ = tx.send(ProbeResult { seq, ip, icmp_type });
                }
                _ => {
                    // println!("[icmp={}] ", icmp_type);
                }
            }
        }
    });

    let mut sent = HashMap::new();
    for ttl in 1..=MAX_HOPS {
        socket.set_ttl(ttl)?;
        println!("{:<2} ttl: ", ttl);

        for probe in 0..args.probes {
            let seq = (ttl * args.probes + probe) as u16;
            let packet = build_icmp_packet(seq, pid);
            let target = SockAddr::from(SocketAddr::new(IpAddr::V4(dest_ip), 0));

            sent.insert(seq, Instant::now());
            socket.send_to(&packet, &target)?;
        }

        let mut reached = false;
        for _ in 0..args.probes {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(result) => {
                    if let Some(start) = sent.get(&result.seq) {
                        let rtt = start.elapsed();

                        if let Ok(hostname) = dns_lookup::lookup_addr(&IpAddr::V4(result.ip)) {
                            println!("{} ({}) {}ms", hostname, result.ip, rtt.as_millis());
                        } else {
                            println!("{} {}ms", result.ip, rtt.as_millis());
                        }
                    }

                    if result.icmp_type == ICMP_ECHO_REPLY {
                        reached = true;
                    }
                }
                Err(_) => {
                    println!("* ");
                }
            }
        }
        println!();

        if reached {
            break;
        }
    }

    Ok(())
}
