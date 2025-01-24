use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::net::UdpSocket;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
// use crate::file_filter::Filter; // Import the Filter struct for connection details

// Constants for packet size and acknowledgment timeout
const PACKET_SIZE: usize = 1024; // Size of each packet to be sent
const ACK_TIMEOUT: Duration = Duration::from_secs(1); // Timeout for acknowledgment reception
static INSTANCE: OnceLock<netData> = OnceLock::new();
pub struct netData {
    dns_web_address : String,
    client_port : String,
}
impl netData {
    fn new() -> Self {
        
        let conf_file : fs::File = match fs::File::open("./config.json"){
            Ok(x) => x,
            Err(e) => {
                panic!("Error: config.json missing or destroyed.\n{}", e)
            }
        };
        let reader = BufReader::new(conf_file);
        let conf : Value = match serde_json::from_reader(reader){
            Ok(x) => x,
            Err(e) => {
                panic!("Error: config.json structure damaged.\n{}", e);
            }
        };
        let f_dns_web_address : String = match &conf["dns_web_address"].as_str() {
            None => {
                panic!("Error: dns_web_address was not a string in config.json");
            }
            Some(x) => x.to_string(),
        };
        let f_client_port : String = match &conf["client_port"].as_str() {
            None => {
                panic!("Error: watch_dir was not a string in config.json");
            }
            Some(x) => x.to_string(),
        };
        netData{
            dns_web_address: f_dns_web_address,
            client_port: f_client_port
        }
    }
    pub fn get_instance() -> &'static netData{
        INSTANCE.get_or_init(|| netData::new())
    }
    pub fn get_dns_web_address(&self) -> &str {
        &self.dns_web_address
    }

    // Getter for client_port
    pub fn get_client_port(&self) -> &str {
        &self.client_port
    }
}
pub fn send_full_contents_of_file(filename: &str) -> io::Result<()> {
    // Create a UDP socket bound to an ephemeral port
    let socket = UdpSocket::bind("0.0.0.0:0").expect("OS unable to bind socket.");

    // Retrieve the Filter instance to access configuration details
    
    // Get configuration details from the Filter instance
    // let dir_to_watch = ();
    let netdata = netData::get_instance();
    let dns_web_address = netdata.get_dns_web_address();
    let client_port = netdata.get_client_port();

    // Print configuration details for debugging purposes
    // println!("Directory to watch: {}", dir_to_watch);
    println!("DNS Web Address: {}", dns_web_address);
    println!("Client Port: {}", client_port);
    println!("updating file: {}", filename);
    // Form the server address using the DNS web address and client port
    let server_address = format!("{}:{}", dns_web_address, client_port);
    // Connect the socket to the server
    socket
        .connect(&server_address)
        .expect("Failed to connect to the server.");

    // Open the file to be sent
    // let new_name = filename.as_str();
    let mut file = File::open(filename).expect(format!("Failed to open {}", filename).as_str());
    // Allocate a buffer for file chunks, reserving 4 bytes for the sequence number
    let mut buffer: [u8; 1020] = [0; PACKET_SIZE - 4];
    // Initialize sequence number for packet identification
    let mut sequence_number: u32 = 0u32;

    println!("Sending File...");

    loop {
        // Read a chunk of data from the file into the buffer
        let bytes_read = file.read(&mut buffer)?;
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

    println!("File sent successfully.");
    Ok(())
}
