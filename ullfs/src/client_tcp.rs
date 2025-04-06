// use std::{borrow::BorrowMut, collections::HashMap, fs, io::{BufRead, BufReader}, net::TcpStream, sync::{OnceLock, RwLock}};

// use serde_json::Value;
// use std::io::{Write, Read};

// use crate::{fileDifs, fileFilter};

use std::{fs, io::Read};

use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::{fileDifs, fileFilter};

// static INSTANCE :OnceLock<Connections> = OnceLock::new();
// struct Connections {
//     addresses: RwLock<HashMap<&'static String, &'static TcpStream>>,
//     connections: u32,
//     port: u32
// }
// impl Connections {
//     pub fn new() -> Self {
        
//         let c = Connections {
//             addresses: RwLock::new(HashMap::new()),
//             connections: 0,
//             port: 0
//         };
//         c.check_connections_config();
//         c
//     }
//     pub fn get_instance() -> &'static Connections{
//         INSTANCE.get_or_init(|| Connections::new())
//     }
// }
pub async fn write_full_file_to_connection(filepath: &str, stream: &mut TcpStream){
    // let mut addr = Connections::get_instance().addresses.write().unwrap();
    // for (address, connection) in addr.iter() {
        // let mut c = *connection;
        // println!("Writing to {}", address);
        // match c.read(&mut []){
        //     Ok(x) => {}
        //     Err(_) => {
        //         Connections::get_instance().check_connections_config();
                
        //     }
        // }
        // println!("c.read complete");
        let base_path = fileFilter::Filter::get_instance().get_base_dir();
        let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
        let mut relative_path_bytes = relative_path.into_bytes();
        relative_path_bytes.push(0b0000);
        // Relative path now ends with a null byte which will never be allowed in a file name
        // This is one of two characters that are completely illegal
        relative_path_bytes.push(1u8);
        // Push identifier for the full file send
        match stream.writable().await {
            Ok(_) => {
                match stream.write_all(relative_path_bytes.as_slice()).await{
                    Ok(x) => x,
                    Err(x) => {
                        println!("Error on writing relative path: {} on connection address",x);
                        return;
                    }
                };
                
                let mut buf = [0u8; 1024];
                let mut f = fs::File::open(filepath).expect(format!("File not found somehow: {}", filepath).as_str());
                let file_length = f.metadata().unwrap().len();
                println!("File length: {}", file_length);
                match stream.write_all(&file_length.to_le_bytes()).await {
                    Err(x) => {
                        println!("Failed to write file length: {}", x);
                        return ;
                    },
                    Ok(_) => {}
                }
                // let mut reader = BufReader::new(f);
                loop {
                    let num_bytes = f.read(&mut buf).expect(format!("Failed to read file: {}", filepath).as_str());
                    match stream.write_all(&buf[..num_bytes]).await{
                        Ok(_) => {},
                        Err(x) => {
                            println!("Failed to write to connection while writing file data. Error: {}", x);
                            return;
                        }
                    };
                    if num_bytes == 0 {
                        break;
                    }
                }
            }
            Err(e) => {
                panic!("Failed to get stream as writable to send full file");
            }
        }
        
        
        
    // }
}
pub async fn write_delta_to_connection(delta: &fileDifs::Delta, filepath: &str, stream: &mut TcpStream){
    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
    println!("{}", relative_path);
    let mut relative_path_bytes = relative_path.into_bytes();
    relative_path_bytes.push(0b0000);
    relative_path_bytes.push(2u8);
    let data_len = delta.data.len();
    println!("Start index: {}", delta.start_index);
    for byte in delta.start_index.to_le_bytes() {
        relative_path_bytes.push(byte);
    }
    println!("End index: {}", delta.end_index);
    
    for byte in delta.end_index.to_le_bytes() {
        relative_path_bytes.push(byte);
    }
    println!("Data length: {}", data_len);
    for byte in data_len.to_le_bytes() {
        relative_path_bytes.push(byte);            
    }
    println!("Hash: {}", delta.old_hash);
    for byte in delta.old_hash.to_le_bytes() {
        println!("Byte: {}", byte);
        
        relative_path_bytes.push(byte);
    }
    for byte in delta.data.iter() {
        relative_path_bytes.push(*byte);
    }
    match stream.writable().await {
        Ok(_) => {
            match stream.write_all(&relative_path_bytes).await {
                Ok(_) =>{}
                Err(x) => {
                    panic!("Failed to write to stream: {}", x);
                }
            }
        }
        Err(x) => {
            panic!("Failed to get stream as writable, error: {}", x);
        }
    };
}