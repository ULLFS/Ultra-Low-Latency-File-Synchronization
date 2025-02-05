use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;
use crate::fileFilter::Filter; // Import the Filter struct for connection details

// Constants for packet size and acknowledgment timeout
const PACKET_SIZE: usize = 1024; // Size of each packet to be sent
const ACK_TIMEOUT: Duration = Duration::from_secs(1); // Timeout for acknowledgment reception

pub fn send_full_contents_of_file_udp(filename: &str) -> io::Result<()> {
    // Create a UDP socket bound to an ephemeral port
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0").expect("OS unable to bind socket.");

    // Retrieve the Filter instance to access configuration details
    let filter: &Filter = Filter::get_instance();
    
    // Get configuration details from the Filter instance
    let dns_web_address: &str = filter.get_dns_web_address();
    let client_port: &str = filter.get_client_port();

    // Form the server address using the DNS web address and client port
    let server_address: String = format!("{}:{}", dns_web_address, client_port);

    // Connect the socket to the server
    socket
        .connect(&server_address)
        .expect("Failed to connect to the server.");

    // Determine the relative file path to send to the server
    let relative_path = Path::new(filename).to_str().expect("Invalid file path");

    // Send the `__SOF__` packet with the relative file path
    let sof_packet = format!("__SOF__{}", relative_path);
    socket.send(sof_packet.as_bytes())?;

    // Open the file to be sent
    let mut file: File = File::open(filename).expect("Failed to open file");

    // Allocate a buffer for file chunks, reserving 4 bytes for the sequence number
    let mut buffer: [u8; 1020] = [0; PACKET_SIZE - 4];

    // Initialize sequence number for packet identification
    let mut sequence_number: u32 = 0u32;

    println!("Sending File over UDP...");

    loop {
        // Read a chunk of data from the file into the buffer
        let bytes_read: usize = file.read(&mut buffer)?;
        if bytes_read == 0 {
            // If no more data, send an EOF packet to signal the end of file transmission
            let eof_packet: &[u8; 7] = b"__EOF__";
            socket.send(eof_packet)?;
            break;
        }

        // Create a packet by adding the sequence number as the first 4 bytes
        let mut packet: Vec<u8> = sequence_number.to_be_bytes().to_vec();
        packet.extend_from_slice(&buffer[..bytes_read]);

        // Retransmission loop to ensure reliable delivery
        loop {
            // Send the packet over the socket
            socket.send(&packet)?;
            let mut ack_buffer = [0; 4]; // Buffer to receive acknowledgment
            socket.set_read_timeout(Some(ACK_TIMEOUT))?; // Set timeout for acknowledgment

            match socket.recv(&mut ack_buffer) {
                Ok(_) if ack_buffer == sequence_number.to_be_bytes() => {
                    // If acknowledgment matches the sequence number, proceed to the next packet
                    break;
                }
                _ => {
                    // If acknowledgment is incorrect or timeout occurs, retransmit the packet
                    eprintln!("Timeout or incorrect ACK, retransmitting sequence: {}", sequence_number);
                }
            }
        }

        // Increment the sequence number, wrapping around if it overflows
        sequence_number = sequence_number.wrapping_add(1);
    }

    println!("File sent successfully over UDP.");
    Ok(())
}

pub fn send_full_contents_of_file_tcp(filename: &str) -> io::Result<()> {
    // Retrieve the Filter instance to access configuration details
    let filter: &Filter = Filter::get_instance();
    let dns_web_address: &str = filter.get_dns_web_address();
    let client_port: &str = filter.get_client_port();

    // Form the server address using the DNS web address and client port
    let server_address: String = format!("{}:{}", dns_web_address, client_port);

    // Establish a TCP connection to the server
    let mut stream = TcpStream::connect(&server_address).expect("Failed to connect to the server.");

    // Determine the relative file path to send to the server
    let relative_path = Path::new(filename).to_str().expect("Invalid file path");

    // Send the `__SOF__` packet with the relative file path
    let sof_packet = format!("__SOF__{}", relative_path);
    stream.write_all(sof_packet.as_bytes())?;

    // Open the file to be sent
    let mut file: File = File::open(filename).expect("Failed to open file");
    let mut buffer = [0; PACKET_SIZE];
    println!("Sending File over TCP...");

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            // Send an EOF marker to signal the end of file transmission
            stream.write_all(b"__EOF__")?;
            break;
        }
        
        // Send the read bytes over the TCP stream
        stream.write_all(&buffer[..bytes_read])?;
    }

    println!("File sent successfully over TCP.");
    Ok(())
}
