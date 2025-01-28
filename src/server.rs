use std::fs::{self, File};
use std::io::{self, Write};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use chrono::Local;
use crate::file_filter::Filter; // Import the Filter struct for connection details

const BUFFER_SIZE: usize = 1024;

pub fn main() -> io::Result<()> {
    // Retrieve the Filter instance to access configuration details
    let filter: &Filter = Filter::get_instance();

    // Get configuration details from the Filter instance
    let server_ip: &str = filter.get_server_ip();
    let server_port: &str = filter.get_server_port();
    let base_dir: &Path = Path::new(filter.get_base_dir());

    // Form the server address using the DNS web address and client port
    let server_address: String = format!("{}:{}", server_ip, server_port);
    let socket: UdpSocket = UdpSocket::bind(server_address)
        .expect("Failed to bind to server address.");

    let mut expected_seq: u32 = 0;
    let mut file_path: Option<PathBuf> = None;

    println!("Waiting for file...");

    loop {
        let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
        let (amt, src) = socket.recv_from(&mut buffer)?;
        if amt > 0 {
            if &buffer[..amt] == b"__EOF__" {
                let timestamp: String = Local::now().format("[%Y-%m-%d %H:%M:%S]").to_string();
                println!("File received successfully. {}", timestamp);
                expected_seq = 0;
                file_path = None; // Reset file path for the next transfer
                continue;
            }

            if &buffer[..amt] == b"exit" {
                break;
            }

            // Handle Start of File packet (__SOF__)
            if buffer.starts_with(b"__SOF__") {
                let relative_path = String::from_utf8_lossy(&buffer[7..amt]).trim().to_string();
                file_path = Some(base_dir.join(relative_path));

                // Ensure directory exists
                if let Some(parent_dir) = file_path.as_ref().and_then(|p| p.parent()) {
                    fs::create_dir_all(parent_dir)?;
                }

                println!("Receiving file: {:?}", file_path.as_ref().unwrap());
                expected_seq = 0;
                continue;
            }

            // Process data packets
            if let Some(ref path) = file_path {
                let seq_num: u32 = u32::from_be_bytes(buffer[..4].try_into().unwrap());
                let data: &[u8] = &buffer[4..amt];
                if seq_num == expected_seq {
                    let mut file = File::options()
                        .create(true)
                        .append(true)
                        .open(path)?;
                    file.write_all(data)?;
                    socket.send_to(&seq_num.to_be_bytes(), src)?;
                    expected_seq = expected_seq.wrapping_add(1);
                } else {
                    // Resend last ACK for out-of-order packets
                    let ack: [u8; 4] = if expected_seq == 0 {
                        [0u8; 4] // Special case for the first packet
                    } else {
                        (expected_seq - 1).to_be_bytes()
                    };
                    socket.send_to(&ack, src)?;
                }
            } else {
                eprintln!("Received data without a valid file path. Ignoring packet.");
            }
        }
    }

    Ok(())
}
