use std::fs::File;
use std::io::{self, Read};
use std::net::UdpSocket;
use std::time::Duration;
use crate::file_filter::Filter; // Import the Filter struct

const PACKET_SIZE: usize = 1024;
const ACK_TIMEOUT: Duration = Duration::from_secs(1);

pub fn send_full_contents_of_file() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("OS unable to bind socket.");

    let filter = Filter::get_instance();
    
    // Use Filter instance values for connection details
    let dir_to_watch = filter.get_base_dir();
    let dns_web_address = filter.get_dns_web_address();
    let client_port = filter.get_client_port();

    // Connect to the server using the values from Filter
    let server_address = format!("{}:{}", dns_web_address, client_port);
    socket
        .connect(&server_address)
        .expect("Failed to connect to the server.");

    let mut file = File::open("test.txt").expect("Failed to open test.txt");
    let mut buffer: [u8; 1020] = [0; PACKET_SIZE - 4]; // Reserve 4 bytes for sequence number
    let mut sequence_number: u32 = 0u32;

    println!("Sending File...");

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            // Send EOF packet
            let eof_packet: &[u8; 7] = b"__EOF__";
            socket.send(eof_packet)?;
            break;
        }

        let mut packet: Vec<u8> = sequence_number.to_be_bytes().to_vec(); // Add sequence number
        packet.extend_from_slice(&buffer[..bytes_read]);

        loop {
            socket.send(&packet)?;
            let mut ack_buffer = [0; 4];
            socket.set_read_timeout(Some(ACK_TIMEOUT))?;

            match socket.recv(&mut ack_buffer) {
                Ok(_) if ack_buffer == sequence_number.to_be_bytes() => {
                    break; // Acknowledgment received
                }
                _ => {
                    eprintln!("Timeout or incorrect ACK, retransmitting sequence: {}", sequence_number);
                }
            }
        }

        sequence_number = sequence_number.wrapping_add(1);
    }

    println!("File sent successfully.");
    Ok(())
}
