use std::fmt::Write;

const PACKET_SIZE: usize = 5;
const CRC_POLY: u32 = 0x04C11DB7;

fn main() {
    let input = std::env::args()
        .nth(1)
        .unwrap_or("Text for CRC demonstration!".to_string());
    println!("Source data: {}", input);

    let packets = split_into_packets(input.as_bytes(), PACKET_SIZE);
    for (i, packet) in packets.iter().enumerate() {
        println!("==============================");
        println!("Packet #{}", i);

        let data_str = String::from_utf8_lossy(packet);
        println!("Data: {}", data_str);

        let crc = crc32(packet);
        let mut encoded = packet.clone();
        encoded.extend_from_slice(&crc.to_be_bytes());

        println!("CRC32: {:08X}", crc);
        println!("Data + CRC32:");
        println!("{}", bytes_to_binary_string(&encoded));

        let mut corrupted = encoded.clone();
        if i % 2 == 1 {
            introduce_error(&mut corrupted, &[3, 17]);
            println!("\n!!! Introduced error in this packet !!!");
            println!("\nData + CRC32 with error:");
            println!("{}", bytes_to_binary_string(&corrupted));
        }

        let received_data = &corrupted[..packet.len()];
        let received_crc = u32::from_be_bytes([
            corrupted[packet.len()],
            corrupted[packet.len() + 1],
            corrupted[packet.len() + 2],
            corrupted[packet.len() + 3],
        ]);
        let calculated_crc = crc32(received_data);

        println!("\nChecking packet...");

        if calculated_crc == received_crc {
            println!("No errors detected");
        } else {
            println!("Errors detected!");
            println!("Expected CRC: {:08X}", received_crc);
            println!("Calculated CRC: {:08X}", calculated_crc);
        }

        println!();
    }
}

fn split_into_packets(data: &[u8], size: usize) -> Vec<Vec<u8>> {
    data.chunks(size).map(|c| c.to_vec()).collect()
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;

    for &byte in data {
        crc ^= (byte as u32) << 24;

        for _ in 0..8 {
            if (crc & 0x80000000) != 0 {
                crc = (crc << 1) ^ CRC_POLY;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

fn introduce_error(data: &mut [u8], bit_positions: &[usize]) {
    for &bit_pos in bit_positions {
        let byte_index = bit_pos / 8;
        let bit_index = bit_pos % 8;

        if byte_index < data.len() {
            data[byte_index] ^= 1 << (7 - bit_index);
        }
    }
}

fn bytes_to_binary_string(data: &[u8]) -> String {
    let mut result = String::new();
    for byte in data {
        write!(&mut result, "{:08b} ", byte).unwrap();
    }
    result
}
