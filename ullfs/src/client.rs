use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

use xxhash_rust::xxh3;

use crate::fileFilter::Filter; // Import the Filter struct for connection details
use crate::fileDifs::FileData;
// Constants for packet size and acknowledgment timeout
const PACKET_SIZE: usize = 1024; // Size of each packet to be sent
const HEADER_SIZE: usize = 32;
const ACK_TIMEOUT: Duration = Duration::from_secs(1); // Timeout for acknowledgment reception




fn create_packet_wrapper(id: u64, cur_packet: u64, num_packets: u64, data: &Vec<u8>) -> Vec<u8>{
    if data.len() > PACKET_SIZE - HEADER_SIZE {
        panic!("Data was too large, data size: {}", data.len());
    }
    let mut buffer: Vec<u8> = Vec::new();
    // First eight bytes are id
    let id_bytes = id.to_be_bytes();
    buffer.push(id_bytes[0]);
    buffer.push(id_bytes[1]);
    buffer.push(id_bytes[2]);
    buffer.push(id_bytes[3]);
    buffer.push(id_bytes[4]);
    buffer.push(id_bytes[5]);
    buffer.push(id_bytes[6]);
    buffer.push(id_bytes[7]);
    let cur_packet_bytes = cur_packet.to_be_bytes();
    buffer.push(cur_packet_bytes[0]);
    buffer.push(cur_packet_bytes[1]);
    buffer.push(cur_packet_bytes[2]);
    buffer.push(cur_packet_bytes[3]);
    buffer.push(cur_packet_bytes[4]);
    buffer.push(cur_packet_bytes[6]);
    buffer.push(cur_packet_bytes[5]);
    buffer.push(cur_packet_bytes[7]);
    // Second eight bytes are number of packets in the sequence
    let num_packets_bytes = num_packets.to_be_bytes();
    buffer.push(num_packets_bytes[0]);
    buffer.push(num_packets_bytes[1]);
    buffer.push(num_packets_bytes[2]);
    buffer.push(num_packets_bytes[3]);
    buffer.push(num_packets_bytes[4]);
    buffer.push(num_packets_bytes[5]);
    buffer.push(num_packets_bytes[6]);
    buffer.push(num_packets_bytes[7]);
    
    for d in data {
        buffer.push(*d);
    }
    let hash = xxh3::xxh3_64(&buffer);
    let hash_bytes = hash.to_be_bytes();
    buffer.push(hash_bytes[0]);
    buffer.push(hash_bytes[1]);
    buffer.push(hash_bytes[2]);
    buffer.push(hash_bytes[3]);
    buffer.push(hash_bytes[4]);
    buffer.push(hash_bytes[5]);
    buffer.push(hash_bytes[6]);
    buffer.push(hash_bytes[7]);
    return buffer;
    
    
}
fn create_delta_packet(start_pos: usize, end_pos: usize, delta: Vec<u8>){
    let mut out_vec: Vec<u8> = Vec::new();
    out_vec.push(1u8); // Byte of 1 means that the packet contains deltas 
    for b in start_pos.to_be_bytes() {
        out_vec.push(b);
    }
    for b in end_pos.to_be_bytes() {
        out_vec.push(b);
    }
    for b in delta {
        out_vec.push(b);
    }
    let mut i = 0;
    let mut v : Vec<u8> = Vec::new();
    let mut count_out    = 0;
    let f_num_packets: f32 = (out_vec.len() as f32) / ((PACKET_SIZE - HEADER_SIZE) as f32);
    let num_packets: u64 = f_num_packets.ceil() as u64;
    for b in out_vec {
        v.push(b);
        if i > PACKET_SIZE - HEADER_SIZE {
            i = 0;
            //TODO: Update id to be an incrementing value over the lifetime of the program
            let packet = create_packet_wrapper(0, count_out, num_packets, &v);
            
            count_out += 1;
            v.clear();
        } else {
            i += 1;
        }
    }
    
    
}
// fn create_full_file_packet(filepath: &str){
//     let mut file = File::open(filepath).expect("File did not exist");
//     let mut first_buffer: [u8; PACKET_SIZE - HEADER_SIZE - 1] = [0; PACKET_SIZE - HEADER_SIZE - 1];
//     let mut buffer: [u8; PACKET_SIZE - HEADER_SIZE] = [0; PACKET_SIZE - HEADER_SIZE];
//     file.read(&mut first_buffer);
//     let i = 1;
//     for b in first_buffer {
//         buffer[i] = b;
//         i += 1;
//     }
//     let p = create_packet_wrapper(0, 0, file.metadata().unwrap().len(), &buffer.to_vec());
//     loop {
//         file.read(&mut buffer);
//         // create_packet_wrapper(0, cur_packet, num_packets, data)
//     }
    
// }
pub fn send_full_contents_of_file(filename: &str) -> io::Result<()> {
    let f = FileData::get_instance();
    let delta = f.get_file_delta(filename);
    println!("File data for {}: {}, {:?}, {}", filename, delta.0, delta.2, delta.1);
    Ok(())
}

// pub fn send_full_contents_of_file_old(filename: &str) -> io::Result<()> {

//     // Create a UDP socket bound to an ephemeral port
//     let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0").expect("OS unable to bind socket.");

//     // Retrieve the Filter instance to access configuration details
//     let filter: &Filter = Filter::get_instance();
    
//     // Get configuration details from the Filter instance
//     let dns_web_address: &str = filter.get_dns_web_address();
//     let client_port: &str = filter.get_client_port();

//     // Form the server address using the DNS web address and client port
//     let server_address: String = format!("{}:{}", dns_web_address, client_port);

//     // Connect the socket to the server
//     socket
//         .connect(&server_address)
//         .expect("Failed to connect to the server.");

//     // Determine the relative file path to send to the server
//     let relative_path = Path::new(filename).to_str().expect("Invalid file path");

//     // Send the `__SOF__` packet with the relative file path
//     let sof_packet = format!("__SOF__{}", relative_path);
//     socket.send(sof_packet.as_bytes())?;

//     // Open the file to be sent
//     let mut file: File = File::open(filename).expect("Failed to open file");

//     // Allocate a buffer for file chunks, reserving 4 bytes for the sequence number
//     let mut buffer: [u8; 1020] = [0; PACKET_SIZE - 4];

//     // Initialize sequence number for packet identification
//     let mut sequence_number: u32 = 0u32;

//     println!("Sending File over UDP...");

//     loop {
//         // Read a chunk of data from the file into the buffer
//         let bytes_read: usize = file.read(&mut buffer)?;
//         if bytes_read == 0 {
//             // If no more data, send an EOF packet to signal the end of file transmission
//             let eof_packet: &[u8; 7] = b"__EOF__";
//             socket.send(eof_packet)?;
//             break;
//         }

//         // Create a packet by adding the sequence number as the first 4 bytes
//         let mut packet: Vec<u8> = sequence_number.to_be_bytes().to_vec();
//         packet.extend_from_slice(&buffer[..bytes_read]);

//         // Retransmission loop to ensure reliable delivery
//         loop {
//             // Send the packet over the socket
//             socket.send(&packet)?;
//             let mut ack_buffer = [0; 4]; // Buffer to receive acknowledgment
//             socket.set_read_timeout(Some(ACK_TIMEOUT))?; // Set timeout for acknowledgment

//             match socket.recv(&mut ack_buffer) {
//                 Ok(_) if ack_buffer == sequence_number.to_be_bytes() => {
//                     // If acknowledgment matches the sequence number, proceed to the next packet
//                     break;
//                 }
//                 _ => {
//                     // If acknowledgment is incorrect or timeout occurs, retransmit the packet
//                     eprintln!("Timeout or incorrect ACK, retransmitting sequence: {}", sequence_number);
//                 }
//             }
//         }

//         // Increment the sequence number, wrapping around if it overflows
//         sequence_number = sequence_number.wrapping_add(1);
//     }

//     println!("File sent successfully over UDP.");
//     Ok(())
// }

pub fn send_full_contents_of_file_tcp(filename: &str) -> io::Result<()> {
    // Retrieve the Filter instance to access configuration details
    // let filter: &Filter = Filter::get_instance();
    
    return Ok(());
    
    // let dns_web_address: &str = filter.get_dns_web_address();
    // let client_port: &str = filter.get_client_port();

    // // Form the server address using the DNS web address and client port
    // let server_address: String = format!("{}:{}", dns_web_address, client_port);

    // // Establish a TCP connection to the server
    // let mut stream = TcpStream::connect(&server_address).expect("Failed to connect to the server.");

    // // Determine the relative file path to send to the server
    // let relative_path = Path::new(filename).to_str().expect("Invalid file path");
    
    // // Send the `__SOF__` packet with the relative file path
    // let sof_packet = format!("__SOF__{}", relative_path);
    // stream.write_all(sof_packet.as_bytes())?;

    // // Open the file to be sent
    // let mut file: File = File::open(filename).expect("Failed to open file");
    // let mut buffer = [0; PACKET_SIZE];
    // println!("Sending File over TCP...");

    // loop {
    //     let bytes_read = file.read(&mut buffer)?;
    //     if bytes_read == 0 {
    //         // Send an EOF marker to signal the end of file transmission
    //         stream.write_all(b"__EOF__")?;
    //         break;
    //     }
        
    //     // Send the read bytes over the TCP stream
    //     stream.write_all(&buffer[..bytes_read])?;
    // }

    // println!("File sent successfully over TCP.");
    // Ok(())
}
