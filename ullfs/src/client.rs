use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::net::UdpSocket;
use std::path::Path;
use std::time::Duration;

const USER_CONFIG_FILE: &str = "client.conf";
const PACKET_SIZE: usize = 1024;
const ACK_TIMEOUT: Duration = Duration::from_secs(1);

pub fn send_full_file() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("OS unable to bind socket.");

    let mut server_details = (String::new(), String::new());

    if let Ok(lines) = read_lines(USER_CONFIG_FILE) {
        for line in lines.map_while(Result::ok) {
            if line.starts_with("SERVER_IP=") {
                server_details.0 = line["SERVER_IP=".len()..].to_string();
            } else if line.starts_with("SERVER_PORT=") {
                server_details.1 = line["SERVER_PORT=".len()..].to_string();
            }
        }
    }

    if server_details.0.is_empty() || server_details.1.is_empty() {
        eprintln!("Error: Missing server details in the configuration file.");
        return Ok(());
    }

    socket
        //.connect(format!("{}:{}", server_details.0, server_details.1))
        .connect("server.homiecourt.xyz:34254")
        .expect("Failed to connect to the server.");


    // Open the file and create a buffer to hold the contents
    let mut file = File::open("test.txt")?;
    let mut buffer = [0; PACKET_SIZE - 4]; // Reserve 4 bytes for sequence number
    let mut sequence_number = 0u32;

    println!("Sending File...");

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            // Send EOF packet
            let eof_packet = b"__EOF__";
            socket.send(eof_packet)?;
            break;
        }

        let mut packet = sequence_number.to_be_bytes().to_vec(); // Add sequence number
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

fn read_lines<P>(filename: P) -> io::Result<io::Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}
